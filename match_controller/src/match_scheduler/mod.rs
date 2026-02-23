use crate::matches::sources::{LogsAndReplays, MatchSource};
use crate::routes::{download_bot, download_bot_data, download_map};
use common::configuration::ac_config::{ACConfig, RunType};
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::{Match, MatchPlayer, MatchRequest};
use common::utilities::zip_utils::zip_directory_to_path;
use common::PlayerNum;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use tracing::{error, info};

pub async fn match_scheduler<M: MatchSource>(settings: &ACConfig, match_source: M) {
    let new_match = match_source.next_match().await.unwrap();

    // The game set is the SC2 map. This SC2-specific logic will be moved to sc2 controller in the next iteration
    let map_name = format!("{}.SC2Map", &new_match.map_name.replace(".SC2Map", ""));

    match settings.run_type {
        RunType::Prepare => prepare_match(&settings, new_match, map_name.clone()).await,
        RunType::Submit => submit_result(&settings, match_source, new_match).await,
    }
}

async fn prepare_match(settings: &ACConfig, new_match: Match, map_name: String) {
    info!(
        "Preparing match - {} vs {}",
        &new_match.players[&PlayerNum::One].name,
        &new_match.players[&PlayerNum::Two].name
    );

    let mut match_request: MatchRequest = new_match.clone().into();
    match_request.map_name = map_name.clone();

    delete_all_signals(&settings).await;

    if !settings.base_website_url.is_empty() {
        if let Err(e) = download_assets(&settings, &new_match, map_name.clone()).await {
            info!("Match could not be prepared: {:?}", e);
            let _ = AiArenaGameResult::new_initialization_error(new_match.match_id).to_json_file();
            return;
        }
    }

    if let Err(e) = match_request.write() {
        info!("Match request could not be written: {:?}", e);
        let _ = AiArenaGameResult::new_initialization_error(new_match.match_id).to_json_file();
    } else {
        info!("Match prepared successfully");
    }
}

async fn submit_result<M: MatchSource>(settings: &ACConfig, match_source: M, new_match: Match) {
    info!(
        "Starting match - {} vs {}",
        &new_match.players[&PlayerNum::One].name,
        &new_match.players[&PlayerNum::Two].name
    );

    let aiarena_game_result;
    let start_time = std::time::Instant::now();

    if check_bots_started(settings).await {
        info!("Match is running...");

        // Wait for the game result as signal for completion of the match
        loop {
            if let Ok(result) = AiArenaGameResult::from_json_file() {
                aiarena_game_result = result;
                break;
            }

            sleep(Duration::from_secs(3)).await;
        }
    } else {
        aiarena_game_result = AiArenaGameResult::new_initialization_error(new_match.match_id);
    }

    info!("Match result: {:?}", &aiarena_game_result);
    info!("Match finished in {:?}", start_time.elapsed());

    let mut logs_and_replays = None;

    if !settings.base_website_url.is_empty() {
        tracing::debug!("Submitting result via AI Arena API");

        check_bots_terminated(&settings).await;

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

async fn delete_all_signals(settings: &ACConfig) {
    // Delete any previous match_result.json file
    AiArenaGameResult::delete_json_file().expect("Failed to delete previous match result");

    // Delete bot 1 signal.exit file if it exists
    let bot1_signal_exit_path = PathBuf::from(&settings.log_root)
        .join("bot-controller-1")
        .join("signal.exit");
    match tokio::fs::remove_file(&bot1_signal_exit_path).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => panic!("Failed to clear signal {:?}: {}", bot1_signal_exit_path, e),
    }

    // Delete bot 2 signal.exit file if it exists
    let bot2_signal_exit_path = PathBuf::from(&settings.log_root)
        .join("bot-controller-2")
        .join("signal.exit");
    match tokio::fs::remove_file(&bot2_signal_exit_path).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => panic!("Failed to clear signal {:?}: {}", bot2_signal_exit_path, e),
    }
}

async fn download_assets(
    settings: &ACConfig,
    new_match: &Match,
    map_name: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let arena_match = new_match.aiarena_match.as_ref().unwrap();
    let map_path = PathBuf::from(&settings.game_directory).join(&map_name);

    tracing::debug!("Downloading map {:?} to {:?}", map_name, map_path);

    let bytes = download_map(&settings, &arena_match)
        .await
        .map_err(|e| format!("{:?}", e))?;
    let mut file = tokio::fs::File::create(map_path).await?;
    file.write_all(&bytes).await?;

    tracing::debug!("Downloading bots and bot data");

    let bot1_data = arena_match.bot1.bot_data.as_ref();
    let bot2_data = arena_match.bot2.bot_data.as_ref();

    let bytes = download_bot(&settings, &arena_match, PlayerNum::One)
        .await
        .map_err(|e| format!("{:?}", e))?;
    let bot_name = &new_match.players[&PlayerNum::One].name;
    let bot_path = PathBuf::from(&settings.bot_directory)
        .join("bot1")
        .join(bot_name);
    let bot_folder = bot_path.as_path();
    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder)?;

    if bot1_data.map_or(false, |s| !s.is_empty()) {
        let bytes = download_bot_data(&settings, &arena_match, PlayerNum::One)
            .await
            .map_err(|e| format!("{:?}", e))?;
        let bot_name = &new_match.players[&PlayerNum::One].name;
        let bot_path = PathBuf::from(&settings.bot_directory)
            .join("bot1")
            .join(bot_name)
            .join("data");
        let bot_folder = bot_path.as_path();
        common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder)?;
    }

    let bytes = download_bot(&settings, &arena_match, PlayerNum::Two)
        .await
        .map_err(|e| format!("{:?}", e))?;
    let bot_name = &new_match.players[&PlayerNum::Two].name;
    let bot_path = PathBuf::from(&settings.bot_directory)
        .join("bot2")
        .join(bot_name);
    let bot_folder = bot_path.as_path();
    common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder)?;

    if bot2_data.map_or(false, |s| !s.is_empty()) {
        let bytes = download_bot_data(&settings, &arena_match, PlayerNum::Two)
            .await
            .map_err(|e| format!("{:?}", e))?;
        let bot_name = &new_match.players[&PlayerNum::Two].name;
        let bot_path = PathBuf::from(&settings.bot_directory)
            .join("bot2")
            .join(bot_name)
            .join("data");
        let bot_folder = bot_path.as_path();
        common::utilities::zip_utils::zip_extract_from_bytes(&bytes, bot_folder)?;
    }

    Ok(())
}

async fn check_bots_started(settings: &ACConfig) -> bool {
    // Check if both bots managed to start
    // Notice: The 2 seconds sleep is not introduced now. It was previously in bot controller.
    // In the next iteration, this will be improved by:
    // - waiting for a signal from game controller that both players joined
    // - removing the 2 seconds sleep, since the signal from game controller will cover for it
    // - result in initialization error only when both bots didn't start to prevent cheating
    sleep(Duration::from_secs(2)).await;

    // Check if bot 1 exited early
    let bot1_signal_exit_path = PathBuf::from(&settings.log_root)
        .join("bot-controller-1")
        .join("signal.exit");
    if bot1_signal_exit_path.exists() {
        return false;
    }

    // Check if bot 2 exited early
    let bot2_signal_exit_path = PathBuf::from(&settings.log_root)
        .join("bot-controller-2")
        .join("signal.exit");
    if bot2_signal_exit_path.exists() {
        return false;
    }

    // No bot exited in 2 seconds. We assume they can join the match.
    true
}

async fn check_bots_terminated(settings: &ACConfig) {
    let bot1_signal_exit_path = PathBuf::from(&settings.log_root)
        .join("bot-controller-1")
        .join("signal.exit");
    let bot2_signal_exit_path = PathBuf::from(&settings.log_root)
        .join("bot-controller-2")
        .join("signal.exit");

    let start_time = std::time::Instant::now();
    loop {
        let bot1_exited = bot1_signal_exit_path.exists();
        let bot2_exited = bot2_signal_exit_path.exists();

        if bot1_exited && bot2_exited {
            return;
        }

        if start_time.elapsed() >= Duration::from_secs(60) {
            if !bot1_exited {
                info!("Bot 1 did not terminate within 60 seconds");
            }
            if !bot2_exited {
                info!("Bot 2 did not terminate within 60 seconds");
            }
            return;
        }

        sleep(Duration::from_secs(1)).await;
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
