use crate::matches::sources::{LogsAndReplays, MatchSource};
use crate::routes::{download_bot, download_bot_data, download_map};
use crate::state::ControllerState;
use common::api::api_reference::bot_controller_client::BotController;
use common::api::api_reference::sc2_controller_client::SC2Controller;
use common::api::api_reference::ControllerApi;
use common::configuration::ac_config::{ACConfig, RunType};
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::{Match, MatchPlayer, MatchRequest};
use common::models::bot_controller::StartBot;
use common::utilities::directory::ensure_directory_structure;
use common::PlayerNum;
use futures_util::future::join3;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::join;
use tokio::time::sleep;
use tracing::{error, info};

pub async fn match_scheduler<M: MatchSource>(
    controller_state: Arc<RwLock<ControllerState>>,
    match_source: M,
) {
    info!("Arena Client started");

    // Delete any previous match_result.json file
    AiArenaGameResult::delete_json_file().expect("Failed to delete previous match result");

    let settings = controller_state.read().settings.clone();

    let mut bot_controllers =
        init_bot_controllers(&settings).expect("Failed to initialize the bot controllers");
    controller_state.write().bot_controllers = bot_controllers.to_vec();

    let sc2_controller = SC2Controller::new(&settings.sc2_cont_host, settings.sc2_cont_port)
        .expect("Failed to create SC2 controller");
    controller_state.write().sc2_controller = Some(sc2_controller.clone());

    start_controllers(&bot_controllers, &sc2_controller).await;

    let new_match = match_source.next_match().await.unwrap();
    let start_time = std::time::Instant::now();

    {
        let mut temp_state = controller_state.write();
        temp_state.current_match = Some(new_match.clone());
    }

    info!(
        "Starting match - {} vs {}",
        &new_match.players[&PlayerNum::One].name,
        &new_match.players[&PlayerNum::Two].name
    );

    if let Err(e) = run_match(
        &settings,
        controller_state.clone(),
        sc2_controller.clone(),
        &mut bot_controllers,
        &new_match,
    )
    .await
    {
        info!("Match failed: {:?}", e);
        let _ = AiArenaGameResult::new_initialization_error(new_match.match_id).to_json_file();
    }

    // Read the resulting match_result.json file
    let aiarena_game_result =
        AiArenaGameResult::from_json_file().expect("Failed to read match result");

    info!("Match result: {:?}", &aiarena_game_result);
    info!("Match finished in {:?}", start_time.elapsed());

    let mut logs_and_replays = None;

    if settings.run_type == RunType::AiArena {
        tracing::debug!("Submitting result to AI Arena");

        logs_and_replays =
            match build_logs_and_replays_object(&new_match, &new_match.players, &settings).await {
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

    let shutdown_sender = controller_state.read().shutdown_sender.clone();
    if let Err(e) = shutdown_sender.send(()).await {
        error!("Failed graceful shutdown. Killing process: {:?}", e);
        std::process::exit(2);
    }
    // todo: Implement clean-up
    // todo: Clean up folders, zip files
}

async fn start_controllers(bot_controllers: &[BotController; 2], sc2_controller: &SC2Controller) {
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
}

async fn run_match(
    settings: &ACConfig,
    controller_state: Arc<RwLock<ControllerState>>,
    sc2_controller: SC2Controller,
    bot_controllers: &mut [BotController],
    new_match: &Match,
) -> Result<(), Box<dyn std::error::Error>> {
    if settings.run_type == RunType::AiArena {
        download_gameset(&settings, controller_state.clone(), &new_match).await?;
    }

    prepare_game(controller_state.clone(), &new_match).await?;

    start_game(sc2_controller.clone()).await?;

    if settings.run_type == RunType::AiArena {
        download_bots(&settings, controller_state.clone(), &new_match).await?;
    }

    start_bots(bot_controllers, &new_match).await?;

    Ok(())
}

async fn download_gameset(
    settings: &ACConfig,
    controller_state: Arc<RwLock<ControllerState>>,
    new_match: &Match,
) -> Result<(), Box<dyn std::error::Error>> {
    let map_name = format!("{}.SC2Map", &new_match.map_name.replace(".SC2Map", ""));
    let map_path = PathBuf::from(&settings.game_directory).join(&map_name);

    tracing::debug!("Downloading map {:?} to {:?}", map_name, map_path);

    let bytes = download_map(controller_state.clone())
        .await
        .map_err(|e| format!("{:?}", e))?;
    let mut file = tokio::fs::File::create(map_path).await?;
    file.write_all(&bytes).await?;

    Ok(())
}

async fn prepare_game(
    controller_state: Arc<RwLock<ControllerState>>,
    new_match: &Match,
) -> Result<(), Box<dyn std::error::Error>> {
    let map_name = format!("{}.SC2Map", &new_match.map_name.replace(".SC2Map", ""));
    controller_state.write().map = Some(map_name.clone());

    info!("Preparing the input to SC2");

    // Write the match request as match_request.toml
    let mut match_request: MatchRequest = new_match.clone().into();
    match_request.map_name = map_name.clone();
    match_request.write()?;

    Ok(())
}

async fn start_game(sc2_controller: SC2Controller) -> Result<(), Box<dyn std::error::Error>> {
    info!("Sending start requests to SC2");
    sc2_controller.start().await?;
    tracing::info!("SC2 started");

    Ok(())
}

async fn download_bots(
    settings: &ACConfig,
    controller_state: Arc<RwLock<ControllerState>>,
    new_match: &Match,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::debug!("Downloading bots and bot data");

    let arena_match = new_match.aiarena_match.as_ref().unwrap();
    let bot1_data = arena_match.bot1.bot_data.as_ref();
    let bot2_data = arena_match.bot2.bot_data.as_ref();

    let bytes = download_bot(controller_state.clone(), PlayerNum::One)
        .await
        .map_err(|e| format!("{:?}", e))?;
    let bot_name = &new_match.players[&PlayerNum::One].name;
    let bot_path = PathBuf::from(&settings.bot_directory)
        .join("bot1")
        .join(bot_name);
    let bot_folder = bot_path.as_path();
    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);

    if bot1_data.map_or(false, |s| !s.is_empty()) {
        let bytes = download_bot_data(controller_state.clone(), PlayerNum::One)
            .await
            .map_err(|e| format!("{:?}", e))?;
        let bot_name = &new_match.players[&PlayerNum::One].name;
        let bot_path = PathBuf::from(&settings.bot_directory)
            .join("bot1")
            .join(bot_name)
            .join("data");
        let bot_folder = bot_path.as_path();
        common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);
    }

    let bytes = download_bot(controller_state.clone(), PlayerNum::Two)
        .await
        .map_err(|e| format!("{:?}", e))?;
    let bot_name = &new_match.players[&PlayerNum::Two].name;
    let bot_path = PathBuf::from(&settings.bot_directory)
        .join("bot2")
        .join(bot_name);
    let bot_folder = bot_path.as_path();
    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);

    if bot2_data.map_or(false, |s| !s.is_empty()) {
        let bytes = download_bot_data(controller_state.clone(), PlayerNum::Two)
            .await
            .map_err(|e| format!("{:?}", e))?;
        let bot_name = &new_match.players[&PlayerNum::Two].name;
        let bot_path = PathBuf::from(&settings.bot_directory)
            .join("bot2")
            .join(bot_name)
            .join("data");
        let bot_folder = bot_path.as_path();
        common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder);
    }

    Ok(())
}

async fn start_bots(
    bot_controllers: &mut [BotController],
    new_match: &Match,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::debug!("Starting bots");

    bot_controllers[0].set_start_bot(create_start_bot(PlayerNum::One, &new_match));
    bot_controllers[1].set_start_bot(create_start_bot(PlayerNum::Two, &new_match));

    match join!(bot_controllers[0].start(), bot_controllers[1].start()) {
        (Ok(_), Ok(_)) => {
            tracing::trace!("Bots started");
        }
        (Err(e), _) => {
            let _ = AiArenaGameResult::new_initialization_error(new_match.match_id).to_json_file();
            return Err(e.into());
        }
        (_, Err(e)) => {
            let _ = AiArenaGameResult::new_initialization_error(new_match.match_id).to_json_file();
            return Err(e.into());
        }
    }

    loop {
        tracing::trace!("Waiting for results");

        if let Ok(_) = AiArenaGameResult::from_json_file() {
            break;
        }

        sleep(Duration::from_secs(3)).await;
    }

    Ok(())
}

async fn build_logs_and_replays_object(
    the_match: &Match,
    players: &HashMap<PlayerNum, MatchPlayer>,
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

    let replay_file = Path::new(&settings.game_directory).join(format!(
        "{}_{}_vs_{}.SC2Replay",
        the_match.match_id,
        players[&PlayerNum::One].name,
        players[&PlayerNum::Two].name
    ));

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

fn create_start_bot(player_num: PlayerNum, new_match: &Match) -> StartBot {
    StartBot {
        bot_name: new_match.players[&player_num].name.clone(),
        bot_type: new_match.players[&player_num].bot_type,
        opponent_id: new_match.players[&player_num.other_player()].id.to_string(),
        player_num,
        match_id: new_match.match_id,
    }
}
