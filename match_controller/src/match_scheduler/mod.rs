use crate::matches::sources::{LogsAndReplays, MatchSource};
use crate::routes::{download_bot, download_bot_data, download_map};
use crate::state::ControllerState;
use common::configuration::ac_config::{ACConfig, RunType};
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::{Match, MatchPlayer, MatchRequest};
use common::utilities::directory::ensure_directory_structure;
use common::utilities::zip_utils::zip_directory_to_path;
use common::PlayerNum;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
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

    if let Err(e) = run_match(&settings, controller_state.clone(), &new_match).await {
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

    info!("Match result submitted");
}

async fn run_match(
    settings: &ACConfig,
    controller_state: Arc<RwLock<ControllerState>>,
    new_match: &Match,
) -> Result<(), Box<dyn std::error::Error>> {
    // The game set is the map
    let map_name = format!("{}.SC2Map", &new_match.map_name.replace(".SC2Map", ""));
    controller_state.write().map = Some(map_name.clone());

    if settings.run_type == RunType::AiArena {
        download_gameset(&settings, controller_state.clone(), map_name.clone()).await?;
    }

    if settings.run_type == RunType::AiArena {
        download_bots(&settings, controller_state.clone(), &new_match).await?;
    }

    start_game(new_match.clone(), map_name).await?;

    Ok(())
}

async fn download_gameset(
    settings: &ACConfig,
    controller_state: Arc<RwLock<ControllerState>>,
    map_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let map_path = PathBuf::from(&settings.game_directory).join(&map_name);

    tracing::debug!("Downloading map {:?} to {:?}", map_name, map_path);

    let bytes = download_map(controller_state.clone())
        .await
        .map_err(|e| format!("{:?}", e))?;
    let mut file = tokio::fs::File::create(map_path).await?;
    file.write_all(&bytes).await?;

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

async fn start_game(new_match: Match, map_name: String) -> Result<(), Box<dyn std::error::Error>> {
    // Write the match request as match_request.toml to trigger the match
    let mut match_request: MatchRequest = new_match.into();
    match_request.map_name = map_name;
    match_request.write()?;

    // Then wait for the game result as signal for completion of the match
    loop {
        if let Ok(_) = AiArenaGameResult::from_json_file() {
            return Ok(());
        }

        sleep(Duration::from_secs(3)).await;
    }
}

async fn build_logs_and_replays_object(
    the_match: &Match,
    players: &HashMap<PlayerNum, MatchPlayer>,
    settings: &ACConfig,
) -> io::Result<LogsAndReplays> {
    let bot1_name = players[&PlayerNum::One].name.clone();
    let bot2_name = players[&PlayerNum::Two].name.clone();

    let bots_folder = Path::new(&settings.bot_directory);
    let logs_folder = Path::new(&settings.log_root);
    let zips_folder = logs_folder.join("zips");

    let ac_zip_path = zips_folder.join("ac_log.zip");
    let bot1_zip_dir = zips_folder.join("bot1");
    let bot2_zip_dir = zips_folder.join("bot2");

    let _ = tokio::fs::remove_dir_all(&zips_folder).await;

    // Zip the log files of all controllers
    zip_directory_for_submit("AC", ac_zip_path.to_path_buf(), logs_folder.to_path_buf());

    // Zip the logs and data of bot 1
    zip_directory_for_submit(
        "bot1 logs",
        bot1_zip_dir.join("logs.zip"),
        bots_folder
            .join("bot1")
            .join(bot1_name.clone())
            .join("logs"),
    );
    zip_directory_for_submit(
        "bot1 data",
        bot1_zip_dir.join("data.zip"),
        bots_folder
            .join("bot1")
            .join(bot1_name.clone())
            .join("data"),
    );

    // Zip the logs and data of bot 2
    zip_directory_for_submit(
        "bot2 logs",
        bot2_zip_dir.join("logs.zip"),
        bots_folder
            .join("bot2")
            .join(bot2_name.clone())
            .join("logs"),
    );
    zip_directory_for_submit(
        "bot2 data",
        bot2_zip_dir.join("data.zip"),
        bots_folder
            .join("bot2")
            .join(bot2_name.clone())
            .join("data"),
    );

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
        bot1_dir: bot1_zip_dir,
        bot2_dir: bot2_zip_dir,
        arenaclient_log: ac_zip_path,
        replay_file,
    })
}

// Zips the contents of the given directory into a zip file with the given zip path
fn zip_directory_for_submit(label: &str, zip_path: PathBuf, directory: PathBuf) {
    println!("ZIP {:?}: {:?} -> {:?}", label, directory, zip_path);
    let result = zip_directory_to_path(&zip_path, &directory);

    match result {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to zip {} logs: {:?}", label, e)
        }
    }
}
