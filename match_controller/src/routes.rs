use bytes::Bytes;
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use common::api::errors::app_error::AppError;
use common::api::errors::download_error::DownloadError;
use common::configuration::ac_config::ACConfig;
use common::models::aiarena::aiarena_match::AiArenaMatch;
use common::PlayerNum;
use tracing::{self, error};

pub async fn download_bot(
    settings: &ACConfig,
    current_match: &AiArenaMatch,
    player_num: PlayerNum,
) -> Result<Bytes, AppError> {
    let api = AiArenaApiClient::new(
        &settings.base_website_url,
        settings.api_token.as_ref().unwrap(),
    )
    .unwrap(); //Would've failed before this point already
    let (source_url, md5_hash, unique_key) = match player_num {
        PlayerNum::One => (
            current_match.bot1.bot_zip.clone(),
            current_match.bot1.bot_zip_md5hash.clone(),
            format!("{}_zip", current_match.bot1.name),
        ),
        PlayerNum::Two => (
            current_match.bot2.bot_zip.clone(),
            current_match.bot2.bot_zip_md5hash.clone(),
            format!("{}_zip", current_match.bot2.name),
        ),
    };
    let mut url = url::Url::parse(&settings.caching_server_url).unwrap();
    url = url.join("/download").unwrap();

    match api
        .download_cached_file(url.as_str(), &source_url, &unique_key, &md5_hash)
        .await
    {
        Ok(x) => Ok(x),
        Err(e) => {
            error!(
                "Cached data download failed, downloading from original source: {:?}",
                e
            );
            api.download_zip(&source_url, !settings.aws)
                .await
                .map_err(|e| AppError::Download(DownloadError::Other(e.to_string())))
        }
    }
}

pub async fn download_bot_data(
    settings: &ACConfig,
    current_match: &AiArenaMatch,
    player_num: PlayerNum,
) -> Result<Bytes, AppError> {
    let api = AiArenaApiClient::new(
        &settings.base_website_url,
        settings.api_token.as_ref().unwrap(),
    )
    .unwrap(); //Would've failed before this point already
    if let Some(source_url) = match player_num {
        PlayerNum::One => current_match.bot1.bot_data.clone(),
        PlayerNum::Two => current_match.bot2.bot_data.clone(),
    } {
        let mut url = url::Url::parse(&settings.caching_server_url).unwrap();
        url = url.join("/download").unwrap();
        let (md5_hash, unique_key) = match player_num {
            PlayerNum::One => (
                current_match.bot1.bot_data_md5hash.clone(),
                format!("{}_data", current_match.bot1.name),
            ),
            PlayerNum::Two => (
                current_match.bot2.bot_data_md5hash.clone(),
                format!("{}_data", current_match.bot2.name),
            ),
        };
        match api
            .download_cached_file(
                url.as_str(),
                &source_url,
                &unique_key,
                md5_hash.unwrap().as_str(),
            )
            .await
        {
            Ok(x) => Ok(x),
            Err(e) => {
                error!(
                    "Cached zip download failed, downloading from original source: {:?}",
                    e
                );
                api.download_zip(&source_url, !settings.aws)
                    .await
                    .map_err(|e| AppError::Download(DownloadError::Other(e.to_string())))
            }
        }
    } else {
        Err(AppError::Download(DownloadError::NotAvailable(
            "No data url for bot".to_string(),
        )))
    }
}

pub async fn download_map(
    settings: &ACConfig,
    current_match: &AiArenaMatch,
) -> Result<Bytes, AppError> {
    let api = AiArenaApiClient::new(
        &settings.base_website_url,
        settings.api_token.as_ref().unwrap(),
    )
    .unwrap(); //Would've failed before this point already
    let source_url = &current_match.map.file;
    let unique_key = &current_match.map.name;
    let mut url = url::Url::parse(&settings.caching_server_url).unwrap();
    url = url.join("/download").unwrap();
    if let Some(md5_hash) = &current_match.map.file_hash {
        match api
            .download_cached_file(url.as_str(), &source_url, &unique_key, &md5_hash)
            .await
        {
            Ok(x) => Ok(x),
            Err(e) => {
                error!(
                    "Cached map download failed, downloading from original source: {:?}",
                    e
                );
                api.download_map(source_url, !settings.aws)
                    .await
                    .map_err(|e| AppError::Download(DownloadError::Other(e.to_string())))
            }
        }
    } else {
        api.download_map(source_url, !settings.aws)
            .await
            .map_err(|e| AppError::Download(DownloadError::Other(e.to_string())))
    }
}
