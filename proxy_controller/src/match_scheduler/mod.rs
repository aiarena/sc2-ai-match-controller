use crate::game::game_config::GameConfig;
use crate::game::game_result::GameResult;
use crate::matches::sources::{LogsAndReplays, MatchSource};
use crate::matches::{Match, MatchPlayer};
use crate::state::{ProxyState, SC2Url};
use bytes::Bytes;
use common::api::api_reference::bot_controller_client::BotController;
use common::api::api_reference::sc2_controller_client::SC2Controller;
use common::api::api_reference::{ApiError, ControllerApi};
use common::configuration::ac_config::{ACConfig, RunType};
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_result::AiArenaResult;
use common::models::bot_controller::StartBot;
use common::utilities::directory::ensure_directory_structure;
use common::utilities::portpicker::Port;
use common::PlayerNum;
use futures_util::future::{join, join3, join4};
use futures_util::TryFutureExt;
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

    let mut bot_controllers = match init_bot_controllers(&settings) {
        Ok(c) => c,
        Err(e) => {
            error!("{}", e);

            std::process::exit(2);
        }
    };
    proxy_state.write().bot_controllers = bot_controllers.to_vec();

    let mut sc2_controllers = match init_sc2_controllers(&settings) {
        Ok(c) => c,
        Err(e) => {
            error!("{}", e);

            std::process::exit(2);
        }
    };
    proxy_state.write().sc2_controllers = sc2_controllers.to_vec();
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
            sc2_controllers[0].health(),
        )
        .await;
        ready = res.0 && res.1 && res.2;
    }

    let _cleanup_res = join3(
        bot_controllers[0].terminate_all("graceful"),
        bot_controllers[1].terminate_all("graceful"),
        sc2_controllers[0].terminate_all("kill"),
    )
    .await;

    if settings.run_type == RunType::AiArena {
        ensure_directory_structure("/", &settings.bots_directory)
            .await
            .expect("Could not create bot path");
    }

    let mut match_counter = 0;
    let rounds_per_run = settings.rounds_per_run;
    let now = std::time::Instant::now();
    'main_loop: while match_source.has_next().await
        && (match_counter < rounds_per_run || rounds_per_run == -1)
    {
        info!("Sending start requests to SC2");

        let response = tokio::spawn(join(
            sc2_controllers[0].clone().start_owned(),
            sc2_controllers[1].clone().start_owned(),
        ));

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

        tracing::trace!("Finding map");
        match sc2_controllers[0].find_map(&new_match.map_name).await {
            Ok(map) => {
                proxy_state.write().map = Some(map.map_path);
            }
            Err(e) => {
                error!("Failed to find map: {}", e);
                break 'main_loop;
            }
        }

        let (process_key1, process_key2) = match response.await.unwrap() {
            (Ok(sc1_resp), Ok(sc2_resp)) => {
                tracing::debug!("SC2-1 Response:\n{:?}", sc1_resp);
                tracing::debug!("SC2-2 Response:\n{:?}", sc2_resp);
                let urls = vec![
                    SC2Url::new(&settings.sc2_cont_host, &sc1_resp),
                    SC2Url::new(&settings.sc2_cont_host, &sc2_resp),
                ];
                tracing::trace!("Adding SC2 urls");
                proxy_state.write().sc2_urls.extend(urls);

                tracing::trace!("SC2 urls added");
                sc2_controllers[0].set_process_key(sc1_resp.process_key);
                proxy_state.write().sc2_controllers[0].set_process_key(sc1_resp.process_key);
                sc2_controllers[1].set_process_key(sc2_resp.process_key);
                proxy_state.write().sc2_controllers[1].set_process_key(sc2_resp.process_key);
                bot_controllers[0].set_process_key(sc1_resp.process_key);
                proxy_state.write().bot_controllers[0].set_process_key(sc1_resp.process_key);
                bot_controllers[1].set_process_key(sc2_resp.process_key);
                proxy_state.write().bot_controllers[1].set_process_key(sc2_resp.process_key);

                (sc1_resp.process_key, sc2_resp.process_key)
            }
            (Err(e), _) | (_, Err(e)) => {
                error!("Failed to start SC2: {}", e);
                break 'main_loop;
            }
        };

        tracing::debug!("Starting bots");
        let mut bots_started = false;
        let should_download = settings.run_type == RunType::AiArena;
        bot_controllers[0].set_start_bot(create_start_bot(
            PlayerNum::One,
            &new_match,
            process_key1,
            should_download,
        ));
        bot_controllers[1].set_start_bot(create_start_bot(
            PlayerNum::Two,
            &new_match,
            process_key2,
            should_download,
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
        // let serialized_result = serde_json::to_value(aiarena_game_result).unwrap();
        info!("{:?}", &aiarena_game_result);
        info!("Match finished in {:?}", start_time.elapsed());
        let game_config = GameConfig::new(&new_match, &settings);
        let logs_and_replays = match build_logs_and_replays_object(
            &new_match.players,
            &bot_controllers,
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

        if let Err(e) = match_source
            .submit_result(&aiarena_game_result, logs_and_replays)
            .await
        {
            error!("{:?}", e);
        }
        match_counter += 1;
        let _cleanup_res = join3(
            bot_controllers[0].terminate_all("graceful"),
            bot_controllers[1].terminate_all("graceful"),
            sc2_controllers[0].terminate_all("kill"),
        )
        .await;
        let mut state = proxy_state.write();
        clean_up_state(&mut state);
    }
    info!("Finished games in {:?}", now.elapsed().as_millis());
    match join3(
        bot_controllers[0].shutdown(),
        bot_controllers[1].shutdown(),
        sc2_controllers[0].shutdown(),
    )
    .await
    {
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            error!("Failed to shutdown one or more controllers: {:?}", e);
        }
        _ => {}
    };
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
    bot_controllers: &[BotController],
    replay_file: PathBuf,
    settings: &ACConfig,
) -> io::Result<LogsAndReplays> {
    let bot1_name = players[&PlayerNum::One].name.clone();
    let bot2_name = players[&PlayerNum::Two].name.clone();
    let temp_folder = Path::new(&settings.temp_root).join(&settings.temp_path);
    let _ = tokio::fs::remove_dir_all(&temp_folder).await;

    ensure_directory_structure(&settings.temp_root, &settings.temp_path).await?;

    let (bot1_dir, bot2_dir) = build_bot_logs(&temp_folder, bot_controllers).await.unwrap();

    let arenaclient_log_directory = build_arenaclient_logs(&temp_folder, bot_controllers)
        .await
        .unwrap(); // todo: dont unwrap

    // Copy proxy_controller logs last to pick up any potential issues
    let proxy_log_path_str = format!(
        "{}/proxy_controller/proxy_controller.log",
        &settings.log_root
    );
    let proxy_log_path = Path::new(&proxy_log_path_str).to_path_buf();

    if proxy_log_path.exists() {
        let _ = tokio::fs::copy(
            proxy_log_path,
            arenaclient_log_directory.join("proxy_controller.log"),
        )
        .await;
    }
    let arenaclient_logs_zip_path = temp_folder.join("ac_log.zip");

    let ac_zip_result = common::utilities::zip_utils::zip_directory_to_path(
        &arenaclient_logs_zip_path,
        &arenaclient_log_directory,
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

async fn build_bot_logs(
    temp_folder: &Path,
    bot_controllers: &[BotController],
) -> io::Result<(PathBuf, PathBuf)> {
    let bot1_dir = temp_folder.join("bot1");
    tokio::fs::create_dir(&bot1_dir).await?;
    let bot2_dir = temp_folder.join("bot2");
    tokio::fs::create_dir(&bot2_dir).await?;
    tokio::fs::create_dir(&bot1_dir.join("logs")).await?;
    tokio::fs::create_dir(&bot2_dir.join("logs")).await?;

    let res = join4(
        bot_controllers[0].download_bot_log().and_then(|x| {
            let file_path = bot1_dir.join("logs").join("stderr.log");
            let archive_file = bot1_dir.join("logs.zip");
            let archive_directory = bot1_dir.join("logs");
            async move {
                match write_file(&file_path, &x).await.map_err(ApiError::from) {
                    Ok(_) => common::utilities::zip_utils::zip_directory_to_path(
                        &archive_file,
                        &archive_directory,
                    )
                    .map_err(ApiError::from),
                    e => e,
                }
            }
        }),
        bot_controllers[1].download_bot_log().and_then(|x| {
            let file_path = bot2_dir.join("logs").join("stderr.log");
            let archive_file = bot2_dir.join("logs.zip");
            let archive_directory = bot2_dir.join("logs");
            async move {
                match write_file(&file_path, &x).await.map_err(ApiError::from) {
                    Ok(_) => common::utilities::zip_utils::zip_directory_to_path(
                        &archive_file,
                        &archive_directory,
                    )
                    .map_err(ApiError::from),
                    e => e,
                }
            }
        }),
        bot_controllers[0].download_bot_data().and_then(|x| {
            let file_path = bot1_dir.join("data.zip");
            async move { write_file(&file_path, &x).await.map_err(ApiError::from) }
        }),
        bot_controllers[1].download_bot_data().and_then(|x| {
            let file_path = bot2_dir.join("data.zip");
            async move { write_file(&file_path, &x).await.map_err(ApiError::from) }
        }),
    )
    .await;

    match res {
        (Err(e), _, _, _) | (_, Err(e), _, _) | (_, _, Err(e), _) | (_, _, _, Err(e)) => {
            error!("{:?}", e);
        }
        _ => {}
    }
    Ok((bot1_dir, bot2_dir))
}

fn init_bot_controllers(settings: &ACConfig) -> Result<[BotController; 2], url::ParseError> {
    Ok([
        BotController::new(&settings.bot_cont_1_host, settings.bot_cont_1_port)?,
        BotController::new(&settings.bot_cont_2_host, settings.bot_cont_2_port)?,
    ])
}

fn init_sc2_controllers(settings: &ACConfig) -> Result<[SC2Controller; 2], url::ParseError> {
    Ok([
        SC2Controller::new(&settings.sc2_cont_host, settings.sc2_cont_port)?,
        SC2Controller::new(&settings.sc2_cont_host, settings.sc2_cont_port)?,
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

async fn build_arenaclient_logs(
    temp_folder: &Path,
    bot_controllers: &[BotController],
) -> io::Result<PathBuf> {
    let arenaclient_logs_dir = temp_folder.join("arenaclient");
    tokio::fs::create_dir(&arenaclient_logs_dir).await?;

    let bot_controller_dir = arenaclient_logs_dir.join("bot");
    tokio::fs::create_dir(&bot_controller_dir).await?;

    let res = join(
        bot_controllers[0].download_controller_log().and_then(|x| {
            let file_path = bot_controller_dir.join("bot_controller1.log");
            async move { write_file(&file_path, &x).await.map_err(ApiError::from) }
        }),
        bot_controllers[1].download_controller_log().and_then(|x| {
            let file_path = bot_controller_dir.join("bot_controller2.log");
            async move { write_file(&file_path, &x).await.map_err(ApiError::from) }
        }),
    )
    .await;
    if let (Err(e), _) | (_, Err(e)) = res {
        error!("{:?}", e)
    }

    Ok(arenaclient_logs_dir)
}

fn create_start_bot(
    player_num: PlayerNum,
    new_match: &Match,
    process_key: Port,
    should_download: bool,
) -> StartBot {
    StartBot {
        bot_name: new_match.players[&player_num].name.clone(),
        bot_type: new_match.players[&player_num].bot_type,
        opponent_id: new_match.players[&player_num.other_player()].id.to_string(),
        player_num,
        match_id: new_match.match_id,
        process_key,
        should_download,
    }
}
