use axum::body::Bytes;
use axum::http::StatusCode;
use reqwest::Client;
use std::io;
use tracing::error;

use common::api::errors::app_error::AppError;
use common::api::errors::download_error::DownloadError;
use common::api::errors::process_error::ProcessError;
use common::{configuration::ac_config::ACConfig, PlayerNum};

pub(crate) fn move_bot_to_internal_dir(
    settings: &ACConfig,
    bot_path: &str,
    player_num: PlayerNum,
) -> io::Result<String> {
    match player_num {
        PlayerNum::One => {
            std::fs::copy(bot_path, &settings.bot1_directory)?;
            Ok(settings.bot1_directory.clone())
        }
        PlayerNum::Two => {
            std::fs::copy(bot_path, &settings.bot2_directory)?;
            Ok(settings.bot2_directory.clone())
        }
    }
}

pub async fn download_and_extract(
    url: &str,
    path: &std::path::Path,
    player_num: &PlayerNum,
    md5_hash_url: &str,
) -> Result<(), AppError> {
    let client = Client::new();

    let bot_zip_bytes = download_zip(&client, url, player_num).await?;

    if let Some(expected_md5_hash) = get_md5hash(&client, md5_hash_url, player_num).await? {
        let actual_md5 = format!("{:x}", md5::compute(&bot_zip_bytes));
        if actual_md5 != expected_md5_hash {
            let msg = format!(
                "Actual md5 hash ({:?}) does not match expected md5 hash({:?}",
                actual_md5, expected_md5_hash
            );
            error!(msg);
            return Err(AppError::Download(DownloadError::Other(msg)));
        }
    }

    if path.exists() {
        let _ = tokio::fs::remove_dir(&path).await;
        let _ = tokio::fs::remove_file(&path).await;
    }

    let zip_result = common::utilities::zip_utils::zip_extract_from_bytes(&bot_zip_bytes, path);

    zip_result
        .map_err(DownloadError::from)
        .map_err(AppError::from)
}

async fn download_zip(
    client: &Client,
    url: &str,
    player_num: &PlayerNum,
) -> Result<Bytes, AppError> {
    let request = client
        .request(reqwest::Method::POST, url)
        .json(player_num)
        .build()
        .map_err(|e| {
            AppError::Process(ProcessError::StartError(format!(
                "Could not build download request: {:?}",
                &e
            )))
        })?;

    let resp = match client.execute(request).await {
        Ok(resp) => resp,
        Err(err) => {
            error!("{:?}", err);
            return Err(ProcessError::StartError(format!(
                "Could not download bot from url: {:?}",
                &url
            ))
            .into());
        }
    };
    let status = resp.status();

    if status.is_client_error() || status.is_server_error() {
        let text = resp.text().await.unwrap_or_else(|_| "Error".to_string());
        return if status == StatusCode::NOT_IMPLEMENTED {
            Err(AppError::Download(DownloadError::NotAvailable(text)))
        } else {
            Err(ProcessError::StartError(format!(
                "Status: {:?}\nCould not download bot from url: {:?}",
                status, &url
            ))
            .into())
        };
    }

    let bot_zip_bytes = resp
        .bytes()
        .await
        .map_err(|e| ProcessError::StartError(format!("{e:?}")))?;
    Ok(bot_zip_bytes)
}

async fn get_md5hash(
    client: &Client,
    url: &str,
    player_num: &PlayerNum,
) -> Result<Option<String>, AppError> {
    let md5_hash_request = client
        .request(reqwest::Method::POST, url)
        .json(player_num)
        .build()
        .map_err(|e| {
            AppError::Process(ProcessError::StartError(format!(
                "Could not build download request: {:?}",
                &e
            )))
        })?;

    let md5_hash_resp = match client.execute(md5_hash_request).await {
        Ok(resp) => resp,
        Err(err) => {
            error!("{:?}", err);
            return Err(ProcessError::StartError(format!(
                "Could not get md5 hash from url: {:?}",
                &url
            ))
            .into());
        }
    };
    let md5_status = md5_hash_resp.status();

    if md5_status.is_client_error() || md5_status.is_server_error() {
        let text = md5_hash_resp
            .text()
            .await
            .unwrap_or_else(|_| "Error".to_string());
        return if md5_status == StatusCode::NOT_IMPLEMENTED {
            Err(AppError::Download(DownloadError::NotAvailable(text)))
        } else {
            Err(ProcessError::StartError(format!(
                "Status: {:?}\nCould not download bot from url: {:?}",
                md5_status, &url
            ))
            .into())
        };
    }
    let body = md5_hash_resp
        .text()
        .await
        .map_err(|e| ProcessError::StartError(format!("{e:?}")))?;
    if body.is_empty() {
        Ok(None)
    } else {
        Ok(Some(body))
    }
}
