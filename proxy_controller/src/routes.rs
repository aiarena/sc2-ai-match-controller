use crate::state::ProxyState;
use axum::extract::State;
use axum::Json;
use bytes::Bytes;
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use common::api::errors::app_error::AppError;
use common::api::errors::download_error::DownloadError;
use common::configuration::ac_config::ACConfig;
use common::PlayerNum;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::{self};

#[tracing::instrument]
pub async fn configuration(
    State(state): State<Arc<RwLock<ProxyState>>>,
) -> Result<Json<ACConfig>, AppError> {
    Ok(Json(state.read().settings.clone()))
}

pub async fn download_bot(
    State(state): State<Arc<RwLock<ProxyState>>>,
    //ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(player_num): Json<PlayerNum>,
) -> Result<Bytes, AppError> {
    // todo: Implement authorization
    // if !state.read().auth_whitelist.contains(&addr) {
    //     return Err(DownloadError::Unauthorized.into());
    // }
    let settings = state.read().settings.clone();

    let current_match = match state
        .read()
        .current_match
        .as_ref()
        .and_then(|x| x.aiarena_match.clone())
    {
        None => {
            return Err(DownloadError::Other("current_match is None".to_string()).into());
        }
        Some(m) => m,
    };
    let api = AiArenaApiClient::new(
        &settings.base_website_url,
        settings.api_token.as_ref().unwrap(),
    )
    .unwrap(); //Would've failed before this point already
    let download_url = match player_num {
        PlayerNum::One => current_match.bot1.bot_zip.clone(),
        PlayerNum::Two => current_match.bot2.bot_zip.clone(),
    };
    api.download_zip(&download_url)
        .await
        .map_err(|e| AppError::Download(DownloadError::Other(e.to_string())))
}

pub async fn get_bot_data_md5(
    State(state): State<Arc<RwLock<ProxyState>>>,
    Json(player_num): Json<PlayerNum>,
) -> Result<String, AppError> {
    let current_match = match state
        .read()
        .current_match
        .as_ref()
        .and_then(|x| x.aiarena_match.clone())
    {
        None => {
            return Err(DownloadError::Other("current_match is None".to_string()).into());
        }
        Some(m) => m,
    };
    match player_num {
        PlayerNum::One => Ok(current_match
            .bot1
            .bot_data_md5hash
            .unwrap_or("".to_string())),
        PlayerNum::Two => Ok(current_match
            .bot2
            .bot_data_md5hash
            .unwrap_or("".to_string())),
    }
}

pub async fn get_bot_zip_md5(
    State(state): State<Arc<RwLock<ProxyState>>>,
    Json(player_num): Json<PlayerNum>,
) -> Result<String, AppError> {
    let current_match = match state
        .read()
        .current_match
        .as_ref()
        .and_then(|x| x.aiarena_match.clone())
    {
        None => {
            return Err(DownloadError::Other("current_match is None".to_string()).into());
        }
        Some(m) => m,
    };
    match player_num {
        PlayerNum::One => Ok(current_match.bot1.bot_zip_md5hash),
        PlayerNum::Two => Ok(current_match.bot2.bot_zip_md5hash),
    }
}

pub async fn download_bot_data(
    State(state): State<Arc<RwLock<ProxyState>>>,
    //ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(player_num): Json<PlayerNum>,
) -> Result<Bytes, AppError> {
    // todo: Implement authorization
    // if !state.read().auth_whitelist.contains(&addr) {
    //     return Err(DownloadError::Unauthorized.into());
    // }
    let settings = state.read().settings.clone();

    let current_match = match state
        .read()
        .current_match
        .as_ref()
        .and_then(|x| x.aiarena_match.clone())
    {
        None => {
            return Err(DownloadError::Other("current_match is None".to_string()).into());
        }
        Some(m) => m,
    };
    let api = AiArenaApiClient::new(
        &settings.base_website_url,
        settings.api_token.as_ref().unwrap(),
    )
    .unwrap(); //Would've failed before this point already
    if let Some(download_url) = match player_num {
        PlayerNum::One => current_match.bot1.bot_data.clone(),
        PlayerNum::Two => current_match.bot2.bot_data.clone(),
    } {
        api.download_zip(&download_url)
            .await
            .map_err(|e| AppError::Download(DownloadError::Other(e.to_string())))
    } else {
        Err(AppError::Download(DownloadError::NotAvailable(
            "No data url for bot".to_string(),
        )))
    }
}

pub async fn download_map(State(state): State<Arc<RwLock<ProxyState>>>) -> Result<Bytes, AppError> {
    let settings = state.read().settings.clone();

    let current_match = match state
        .read()
        .current_match
        .as_ref()
        .and_then(|x| x.aiarena_match.clone())
    {
        None => {
            return Err(DownloadError::Other("current_match is None".to_string()).into());
        }
        Some(m) => m,
    };
    let api = AiArenaApiClient::new(
        &settings.base_website_url,
        settings.api_token.as_ref().unwrap(),
    )
    .unwrap(); //Would've failed before this point already
    let map_url = &current_match.map.file;
    api.download_map(map_url)
        .await
        .map_err(|e| AppError::Download(DownloadError::Other(e.to_string())))
}
