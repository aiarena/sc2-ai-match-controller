use crate::game::game_config::GameConfig;
use crate::game::game_result::GameResult;
use crate::matches::sources::{LogsAndReplays, MatchSource};
use crate::matches::{Match, MatchPlayer};
use crate::routes::{download_bot, download_bot_data, download_map};
use crate::state::{ProxyState, SC2Url};
use bytes::Bytes;
use common::api::api_reference::bot_controller_client::BotController;
use common::api::api_reference::sc2_controller_client::SC2Controller;
use common::api::api_reference::ControllerApi;
use common::configuration::ac_config::{ACConfig, RunType};
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_result::AiArenaResult;
use common::models::bot_controller::StartBot;
use common::utilities::directory::ensure_directory_structure;
use common::utilities::portpicker::Port;
use common::PlayerNum;
use futures_util::future::join3;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::join;
use tokio::time::sleep;
use tracing::{error, info};

pub async fn match_scheduler<M: MatchSource>(
    proxy_state: Arc<RwLock<ProxyState>>,
    match_source: M,
) {
    info!("Arena Client started");

    let settings = proxy_state.read().settings.clone();

    let mut bot_controllers =
        init_bot_controllers(&settings).expect("Failed to initialize the bot controllers");
    proxy_state.write().bot_controllers = bot_controllers.to_vec();

    let sc2_controller = SC2Controller::new(&settings.sc2_cont_host, settings.sc2_cont_port)
        .expect("Failed to create SC2 controller");
    proxy_state.write().sc2_controller = Some(sc2_controller.clone());

    // TODO: Enable when auth is implemented
    // let sock_addrs = vec![
    //     bot_controllers[0].sock_addr(),
    //     bot_controllers[1].sock_addr(),
    // ];
    // proxy_state.write().auth_whitelist.extend(sock_addrs);

    info!("Waiting for controllers to become ready");
    let mut ready = false;

    while !ready {
        sleep(Duration::from_millis(500)).await;
        let res = join3(
            bot_controllers[0].health(),
            bot_controllers[1].health(),
            sc2_controller.health(),
        )
        .await;
        ready = res.0 && res.1 && res.2;
    }

    let mut match_counter = 0;
    let rounds_per_run = settings.rounds_per_run;
    let now = std::time::Instant::now();
    'main_loop: while match_source.has_next().await
        && (match_counter < rounds_per_run || rounds_per_run == -1)
    {
        let new_match = match match_source.next_match().await {
            None => {
                sleep(Duration::from_secs(30)).await;
                continue;
            }
            Some(m) => m,
        };

        let start_time = std::time::Instant::now();

        {
            let mut temp_state = proxy_state.write();
            temp_state.current_match = Some(new_match.clone());
            temp_state.game_result = Some(GameResult::new(new_match.match_id));
        }
        info!("Starting Game - Round {}", match_counter);
        info!(
            "{} vs {}",
            &new_match.players[&PlayerNum::One].name,
            &new_match.players[&PlayerNum::Two].name
        );

        let map_name = format!("{}.SC2Map", &new_match.map_name.replace(".SC2Map", ""));
        proxy_state.write().map = Some(map_name.clone());

        if settings.run_type == RunType::AiArena {
            let map_path = PathBuf::from(&settings.game_directory).join(&map_name);

            tracing::debug!("Downloading map {:?} to {:?}", map_name, map_path);

            match download_map(proxy_state.clone()).await {
                Ok(bytes) => match tokio::fs::File::create(map_path).await {
                    Ok(mut file) => {
                        if let Err(e) = file.write_all(&bytes).await {
                            error!("Failed to store map: {:?}", e);
                            break 'main_loop;
                        }
                    }
                    Err(e) => {
                        error!("Failed to store map: {:?}", e);
                        break 'main_loop;
                    }
                },
                Err(e) => {
                    error!("Failed to download map: {:?}", e);
                    break 'main_loop;
                }
            }
        }

        info!("Sending start requests to SC2");
        let (process_key1, process_key2) = match sc2_controller.start().await {
            Ok(responses) if responses.len() == 2 => {
                let sc1_resp = &responses[0];
                let sc2_resp = &responses[1];
                let urls = vec![
                    SC2Url::new(&settings.sc2_cont_host, &sc1_resp),
                    SC2Url::new(&settings.sc2_cont_host, &sc2_resp),
                ];

                tracing::debug!("SC2 listens on {:?} and {:?}", &urls[0], &urls[1]);
                proxy_state.write().sc2_urls.extend(urls);

                bot_controllers[0].set_process_key(sc1_resp.process_key);
                proxy_state.write().bot_controllers[0].set_process_key(sc1_resp.process_key);
                bot_controllers[1].set_process_key(sc2_resp.process_key);
                proxy_state.write().bot_controllers[1].set_process_key(sc2_resp.process_key);

                (sc1_resp.process_key, sc2_resp.process_key)
            }
            Err(e) => {
                error!("Failed to start SC2: {}", e);
                break 'main_loop;
            }
            _ => {
                error!("Unexpected response from SC2 start");
                break 'main_loop;
            }
        };

        if settings.run_type == RunType::AiArena {
            tracing::debug!("Downloading bots and bot data");

            match download_bot(proxy_state.clone(), PlayerNum::One).await {
                Ok(bytes) => {
                    let bot_name = &new_match.players[&PlayerNum::One].name;
                    let bot_path = PathBuf::from(&settings.bot_directory)
                        .join("bot1")
                        .join(bot_name);
                    let bot_folder = bot_path.as_path();
                    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);
                }
                Err(e) => {
                    error!("Failed to download bot 1: {:?}", e);
                    break 'main_loop;
                }
            }

            match download_bot_data(proxy_state.clone(), PlayerNum::One).await {
                Ok(bytes) => {
                    let bot_name = &new_match.players[&PlayerNum::One].name;
                    let bot_path = PathBuf::from(&settings.bot_directory)
                        .join("bot1")
                        .join(bot_name)
                        .join("data");
                    let bot_folder = bot_path.as_path();
                    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);
                }
                Err(e) => {
                    error!("Failed to download bot 1 data: {:?}", e);
                }
            }

            match download_bot(proxy_state.clone(), PlayerNum::Two).await {
                Ok(bytes) => {
                    let bot_name = &new_match.players[&PlayerNum::Two].name;
                    let bot_path = PathBuf::from(&settings.bot_directory)
                        .join("bot2")
                        .join(bot_name);
                    let bot_folder = bot_path.as_path();
                    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);
                }
                Err(e) => {
                    error!("Failed to download bot 2: {:?}", e);
                    break 'main_loop;
                }
            }

            match download_bot_data(proxy_state.clone(), PlayerNum::Two).await {
                Ok(bytes) => {
                    let bot_name = &new_match.players[&PlayerNum::Two].name;
                    let bot_path = PathBuf::from(&settings.bot_directory)
                        .join("bot2")
                        .join(bot_name)
                        .join("data");
                    let bot_folder = bot_path.as_path();
                    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);
                }
                Err(e) => {
                    error!("Failed to download bot 2 data: {:?}", e);
                }
            }
        }

        tracing::debug!("Starting bots");
        let mut bots_started = false;
        bot_controllers[0].set_start_bot(create_start_bot(
            PlayerNum::One,
            &new_match,
            process_key1,
        ));
        bot_controllers[1].set_start_bot(create_start_bot(
            PlayerNum::Two,
            &new_match,
            process_key2,
        ));

        match join!(bot_controllers[0].start(), bot_controllers[1].start()) {
            (Ok(resp1), Ok(resp2)) => {
                tracing::trace!("Bots started");
                let mut counter = 0;
                let max_retries = 60;
                let (mut bot_added1, mut bot_added2) = (false, false);
                while counter < max_retries && !(bot_added1 && bot_added2) {
                    if counter > 0 {
                        sleep(Duration::from_millis(500)).await;
                    }
                    if !bot_added1 {
                        tracing::trace!("Adding bot1");
                        bot_added1 = proxy_state.write().update_player(
                            resp1.port,
                            &new_match.players[&PlayerNum::One].name,
                            PlayerNum::One,
                        );
                    }
                    if !bot_added2 {
                        tracing::trace!("Adding bot2");
                        bot_added2 = proxy_state.write().update_player(
                            resp2.port,
                            &new_match.players[&PlayerNum::Two].name,
                            PlayerNum::Two,
                        );
                    }

                    counter += 1;
                }
                bots_started = true;
            }
            (Err(e), _) => {
                error!("Failed to start bot 1: {}", e);
                proxy_state.write().game_result.as_mut().unwrap().result =
                    Some(AiArenaResult::InitializationError);
            }
            (_, Err(e)) => {
                error!("Failed to start bot 2: {}", e);
                proxy_state.write().game_result.as_mut().unwrap().result =
                    Some(AiArenaResult::InitializationError);
            }
        }
        if bots_started {
            loop {
                tracing::trace!("Waiting for results");
                let (p1_result_ready, p2_result_ready) = {
                    let game_result = { proxy_state.read().game_result.clone().unwrap() };
                    let result_ready = game_result.result.is_some();
                    (
                        result_ready || game_result.player1_result.as_ref().is_some(),
                        result_ready || game_result.player2_result.as_ref().is_some(),
                        // game_result.result.as_ref().is_some(),
                    )
                };

                if p1_result_ready && p2_result_ready {
                    break;
                }

                sleep(Duration::from_secs(3)).await;
            }
        }

        let game_result = proxy_state.read().game_result.clone().unwrap();

        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let mut logs_and_replays = None;
        // let serialized_result = serde_json::to_value(aiarena_game_result).unwrap();
        info!("{:?}", &aiarena_game_result);
        info!("Match finished in {:?}", start_time.elapsed());

        if settings.run_type == RunType::AiArena {
            tracing::debug!("Submitting result to AI Arena");

            let game_config = GameConfig::new(&new_match, &settings);
            logs_and_replays = match build_logs_and_replays_object(
                &new_match.players,
                PathBuf::from(game_config.replay_path()).join(&game_config.replay_name),
                &settings,
            )
            .await
            {
                Ok(l) => Some(l),
                Err(err) => {
                    error!("{:?}", err);
                    None
                }
            };
        }

        if let Err(e) = match_source
            .submit_result(&aiarena_game_result, logs_and_replays)
            .await
        {
            error!("{:?}", e);
        }

        match_counter += 1;

        let mut state = proxy_state.write();
        clean_up_state(&mut state);
    }
    info!("Finished games in {:?}", now.elapsed().as_millis());

    let shutdown_sender = proxy_state.read().shutdown_sender.clone();
    if let Err(e) = shutdown_sender.send(()).await {
        error!("Failed graceful shutdown. Killing process: {:?}", e);
        std::process::exit(2);
    }
    // todo: Implement clean-up
    // todo: Clean up folders, zip files
}

async fn build_logs_and_replays_object(
    players: &HashMap<PlayerNum, MatchPlayer>,
    replay_file: PathBuf,
    settings: &ACConfig,
) -> io::Result<LogsAndReplays> {
    let bot1_name = players[&PlayerNum::One].name.clone();
    let bot2_name = players[&PlayerNum::Two].name.clone();
    let temp_folder = Path::new(&settings.temp_root).join(&settings.temp_path);
    let _ = tokio::fs::remove_dir_all(&temp_folder).await;

    ensure_directory_structure(&settings.temp_root, &settings.temp_path).await?;

    // Zip logs and data of bot 1
    let bot1_dir = temp_folder.join("bot1");
    let bot1_path = Path::new(&settings.bot_directory)
        .join("bot1")
        .join(bot1_name.clone());
    let bot1_logs = bot1_path.join("logs");
    let bot1_logs_zip_path = bot1_dir.join("logs.zip");
    let bot1_logs_zip_result =
        common::utilities::zip_utils::zip_directory_to_path(&bot1_logs_zip_path, &bot1_logs);
    match bot1_logs_zip_result {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to zip bot 1 logs: {:?}", e)
        }
    }
    let bot1_data = bot1_path.join("data");
    let bot1_data_zip_path = bot1_dir.join("data.zip");
    let bot1_data_zip_result =
        common::utilities::zip_utils::zip_directory_to_path(&bot1_data_zip_path, &bot1_data);
    match bot1_data_zip_result {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to zip bot 1 data: {:?}", e)
        }
    }

    // Zip logs and data of bot 2
    let bot2_dir = temp_folder.join("bot2");
    let bot2_path = Path::new(&settings.bot_directory)
        .join("bot2")
        .join(bot2_name.clone());
    let bot2_logs = bot2_path.join("logs");
    let bot2_logs_zip_path = bot2_dir.join("logs.zip");
    let bot2_logs_zip_result =
        common::utilities::zip_utils::zip_directory_to_path(&bot2_logs_zip_path, &bot2_logs);
    match bot2_logs_zip_result {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to zip bot 2 logs: {:?}", e)
        }
    }
    let bot2_data = bot2_path.join("data");
    let bot2_data_zip_path = bot2_dir.join("data.zip");
    let bot2_data_zip_result =
        common::utilities::zip_utils::zip_directory_to_path(&bot2_data_zip_path, &bot2_data);
    match bot2_data_zip_result {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to zip bot 2 data: {:?}", e)
        }
    }

    // Zip all controller log files into a single zip file
    let log_root_path = Path::new(&settings.log_root);
    let arenaclient_logs_zip_path = temp_folder.join("ac_log.zip");
    let ac_zip_result = common::utilities::zip_utils::zip_directory_to_path(
        &arenaclient_logs_zip_path,
        &log_root_path,
    );
    match ac_zip_result {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to zip AC logs: {:?}", e)
        }
    }

    Ok(LogsAndReplays {
        upload_url: format!("{}/upload", &settings.caching_server_url),
        bot1_name,
        bot2_name,
        bot1_dir,
        bot2_dir,
        arenaclient_log: arenaclient_logs_zip_path,
        replay_file,
    })
}

fn init_bot_controllers(settings: &ACConfig) -> Result<[BotController; 2], url::ParseError> {
    Ok([
        BotController::new(&settings.bot_cont_1_host, settings.bot_cont_1_port)?,
        BotController::new(&settings.bot_cont_2_host, settings.bot_cont_2_port)?,
    ])
}

fn clean_up_state(state: &mut ProxyState) {
    state.map = None;
    state.game_result = None;
    state.current_match = None;
    state.sc2_urls.clear();
    state.ready = false;
    state.port_config = None;
    state.auth_whitelist.clear();
    state.game_config = None;
    state.remove_all_clients();
}

async fn write_file(path: &Path, bytes: &Bytes) -> std::io::Result<()> {
    let mut file = File::create(path).await?;
    file.write_all(bytes.as_ref()).await
}

fn create_start_bot(player_num: PlayerNum, new_match: &Match, process_key: Port) -> StartBot {
    StartBot {
        bot_name: new_match.players[&player_num].name.clone(),
        bot_type: new_match.players[&player_num].bot_type,
        opponent_id: new_match.players[&player_num.other_player()].id.to_string(),
        player_num,
        match_id: new_match.match_id,
        process_key,
    }
}
