mod cache;
mod result;
mod signals;
mod store;
mod upload_url;

use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::Path;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

use crate::request::MatchRequest;
use crate::result::MatchResult;
use crate::settings::Settings;

pub async fn submit_result(settings: &Settings) {
    let match_request = match MatchRequest::read_from_file() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to read match request: {}", e);
            return;
        }
    };

    info!(
        "Match {} - {} vs {} on {} - collecting results...",
        &match_request.match_id, &match_request.player_1_name, &match_request.player_2_name, &match_request.map_name
    );
    let match_result = wait_for_match_result(settings, &match_request).await;

    if settings.should_use_arena_api() {
        info!("Waiting for bots to finalize their logs");
        signals::wait_for_bots_to_terminate(settings).await;

        info!("Submitting result via API");
        upload_assets(&match_request, &match_result, settings).await;

        info!("Match result submitted successfully");
    }
}

async fn wait_for_match_result(settings: &Settings, match_request: &MatchRequest) -> MatchResult {
    let start_time = std::time::Instant::now();

    let match_result = if signals::check_bots_started(settings).await {
        info!("Match is running...");
        loop {
            if let Ok(result) = MatchResult::read_from_file() {
                break result;
            }
            sleep(Duration::from_secs(3)).await;
        }
    } else {
        MatchResult::new_initialization_error(match_request.match_id)
    };

    info!("Match result: {:?}", &match_result);
    info!("Match finished in {:?}", start_time.elapsed());

    match_result
}

pub async fn upload_assets(match_request: &MatchRequest, match_result: &MatchResult, settings: &Settings) {
    let bot1_name = match_request.player_1_name.clone();
    let bot2_name = match_request.player_2_name.clone();
    let bots_folder = Path::new(&settings.bot_directory);
    let logs_folder = Path::new(&settings.log_root);
    let replay_file = Path::new(&settings.game_directory).join(format!("{}_{}_vs_{}.SC2Replay", match_request.match_id, bot1_name, bot2_name));

    let replay_id = match store::upload_file(settings, &replay_file).await {
        Ok(id) => id,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };
    let bot1_data_id = match store::upload_zip(settings, &bots_folder.join("bot1").join(&bot1_name).join("data"), settings.should_use_cache()).await {
        Ok(id) => id,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };
    let bot2_data_id = match store::upload_zip(settings, &bots_folder.join("bot2").join(&bot2_name).join("data"), settings.should_use_cache()).await {
        Ok(id) => id,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };
    let bot1_log_id = match store::upload_zip(settings, &bots_folder.join("bot1").join(&bot1_name).join("logs"), false).await {
        Ok(id) => id,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };
    let bot2_log_id = match store::upload_zip(settings, &bots_folder.join("bot2").join(&bot2_name).join("logs"), false).await {
        Ok(id) => id,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };
    let arenaclient_log_id = match store::upload_zip(settings, logs_folder, false).await {
        Ok(id) => id,
        Err(e) => {
            error!("{:?}", e);
            return;
        }
    };

    let input = result::SubmitResultInput {
        match_id: STANDARD.encode(format!("MatchType:{}", match_result.match_id)),
        result_type: match_result.result.to_string(),
        game_steps: match_result.game_steps,
        bot1_avg_step_time: match_result.bot1_avg_step_time.unwrap_or(0.0),
        bot2_avg_step_time: match_result.bot2_avg_step_time.unwrap_or(0.0),
        bot1_tags: match_result.bot1_tags.clone().unwrap_or_default(),
        bot2_tags: match_result.bot2_tags.clone().unwrap_or_default(),
        replay_file: replay_id,
        arenaclient_log: arenaclient_log_id,
        bot1_data: bot1_data_id,
        bot2_data: bot2_data_id,
        bot1_log: bot1_log_id,
        bot2_log: bot2_log_id,
    };

    if let Err(e) = result::submit_result(settings, &input).await {
        error!("Failed to submit result via GraphQL: {}", e);
    }
}
