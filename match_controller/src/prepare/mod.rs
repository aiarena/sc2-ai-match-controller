mod cache;
mod request;
mod signals;
mod store;

use request::DownloadLinks;
use std::path::PathBuf;
use tracing::info;

use crate::request::MatchRequest;
use crate::result::MatchResult;
use crate::settings::Settings;

pub async fn prepare_match(settings: &Settings) {
    signals::delete_all_signals(settings).await;

    let match_request: MatchRequest = if settings.should_use_arena_api() {
        info!("Reading match from API");
        let (match_request, download_links) = request::fetch_match_request(settings).await.expect("Failed to read match from API");
        if let Err(e) = download_assets(settings, &match_request, &download_links).await {
            info!("Match could not be prepared: {:?}", e);
            let _ = match_request.write_to_file();
            let _ = MatchResult::new_initialization_error(match_request.match_id).write_to_file();
            return;
        }
        match_request
    } else {
        info!("Reading match from file");
        MatchRequest::read_from_line(&settings.matches_file).expect("Failed to read match from file")
    };

    if let Err(e) = match_request.write_to_file() {
        info!("Match request could not be written: {:?}", e);
    } else {
        info!("Match {} - {} vs {} on {} - prepared successfully", match_request.match_id, match_request.player_1_name, match_request.player_2_name, match_request.map_name);
    }
}

async fn download_assets(settings: &Settings, match_request: &MatchRequest, links: &DownloadLinks) -> anyhow::Result<()> {
    let map_path = PathBuf::from(&settings.game_directory).join(&match_request.map_name);
    info!("Downloading map {:?} to {:?}", match_request.map_name, map_path);
    store::download_file(settings, &links.map, &format!("map/{}", match_request.map_name), &map_path).await?;

    let bot_path = PathBuf::from(&settings.bot_directory).join("bot1").join(&match_request.player_1_name);
    info!("Downloading bot {:?} to {:?}", match_request.player_1_name, bot_path);
    store::download_zip(settings, &links.bot1_zip, &format!("bot-code/{}", match_request.player_1_name), &bot_path).await?;
    if links.bot1_data.as_ref().is_some_and(|s| !s.is_empty()) {
        store::download_zip(settings, links.bot1_data.as_ref().unwrap(), &format!("bot-data/{}", match_request.player_1_name), &bot_path.join("data")).await?;
    }

    let bot_path = PathBuf::from(&settings.bot_directory).join("bot2").join(&match_request.player_2_name);
    info!("Downloading bot {:?} to {:?}", match_request.player_2_name, bot_path);
    store::download_zip(settings, &links.bot2_zip, &format!("bot-code/{}", match_request.player_2_name), &bot_path).await?;
    if links.bot2_data.as_ref().is_some_and(|s| !s.is_empty()) {
        store::download_zip(settings, links.bot2_data.as_ref().unwrap(), &format!("bot-data/{}", match_request.player_2_name), &bot_path.join("data")).await?;
    }

    Ok(())
}
