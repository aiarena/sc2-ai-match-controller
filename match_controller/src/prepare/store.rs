use bytes::Bytes;
use reqwest::Client;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tracing::{debug, error, info};

use super::cache::download_cache;
use crate::settings::Settings;

pub async fn download_file(settings: &Settings, url: &str, name: &str, file: &Path) -> anyhow::Result<()> {
    let buf = download_data(settings, url, name).await?;
    let mut file = tokio::fs::File::create(file).await?;
    file.write_all(&buf).await?;
    Ok(())
}

pub async fn download_zip(settings: &Settings, url: &str, name: &str, directory: &Path) -> anyhow::Result<()> {
    let buf = download_data(settings, url, name).await?;
    zip_extract_from_bytes(&buf, directory)
}

async fn download_data(settings: &Settings, url: &str, name: &str) -> anyhow::Result<Bytes> {
    if !settings.should_use_cache() {
        return download_from_url(url).await;
    }

    let etag = match get_etag(url).await {
        Ok(e) => e,
        Err(e) => {
            error!("No ETag, downloading from store: {:?}", e);
            return download_from_url(url).await;
        }
    };

    match download_cache(settings, url, name, &etag).await {
        Ok(bytes) => Ok(bytes),
        Err(e) => {
            error!("No cache, downloading from store: {:?}", e);
            download_from_url(url).await
        }
    }
}

async fn get_etag(url: &str) -> anyhow::Result<String> {
    let mut last_err = None;
    for attempt in 1..=10 {
        let start = std::time::Instant::now();
        let response = match Client::new().head(url).send().await {
            Ok(r) => r,
            Err(e) => {
                info!("[http] failure download store 0.000 MB in {:.3}s attempt {}", start.elapsed().as_secs_f64(), attempt);
                error!("ETag attempt {}/10 failed: {}", attempt, e);
                last_err = Some(anyhow::Error::from(e));
                if attempt < 10 {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                continue;
            }
        };
        let status = response.status();

        if status.is_success() {
            info!("[http] success download store 0.000 MB in {:.3}s attempt {}", start.elapsed().as_secs_f64(), attempt);
            return Ok(response.headers().get(reqwest::header::ETAG).and_then(|v| v.to_str().ok()).unwrap_or("").to_string());
        }
        info!("[http] failure download store 0.000 MB in {:.3}s attempt {}", start.elapsed().as_secs_f64(), attempt);
        if !status.is_server_error() {
            return Err(anyhow::anyhow!("ETag request failed: {}", status));
        }
        error!("ETag attempt {}/10 server error {}: retrying", attempt, status);
        last_err = Some(anyhow::anyhow!("ETag server error: {}", status));
        if attempt < 10 {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
    Err(last_err.unwrap())
}

async fn download_from_url(url: &str) -> anyhow::Result<Bytes> {
    let mut last_err = None;
    for attempt in 1..=10 {
        let start = std::time::Instant::now();
        let response = match Client::new().get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                info!("[http] failure download store 0.000 MB in {:.3}s attempt {}", start.elapsed().as_secs_f64(), attempt);
                error!("Download attempt {}/10 failed: {}", attempt, e);
                last_err = Some(anyhow::Error::from(e));
                if attempt < 10 {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                continue;
            }
        };
        let status = response.status();

        if status.is_success() {
            let bytes = response.bytes().await.map_err(anyhow::Error::from)?;
            info!(
                "[http] success download store {:.3} MB in {:.3}s attempt {}",
                bytes.len() as f64 / 1_000_000.0,
                start.elapsed().as_secs_f64(),
                attempt
            );
            return Ok(bytes);
        }
        info!("[http] failure download store 0.000 MB in {:.3}s attempt {}", start.elapsed().as_secs_f64(), attempt);
        if !status.is_server_error() {
            return Err(anyhow::anyhow!("Download failed: {}", status));
        }
        error!("Download attempt {}/10 server error {}: retrying", attempt, status);
        last_err = Some(anyhow::anyhow!("Download server error: {}", status));
        if attempt < 10 {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
    Err(last_err.unwrap())
}

fn zip_extract_from_bytes(buf: &Bytes, directory: &Path) -> anyhow::Result<()> {
    let mut tmp_file = tempfile::NamedTempFile::new().unwrap();
    tmp_file.write_all(buf).unwrap();

    let new_file = tmp_file.into_temp_path();
    let path = new_file.to_string_lossy().to_string();

    let mut command = Command::new("7z");
    command.arg("x").arg(&path).arg(format!("-o{}", directory.to_string_lossy())).arg("-r").arg("-tzip").arg("-y");
    debug!("{:?}", command);
    let process = command.output()?;

    let exit_status = process.status;

    if exit_status.success() {
        Ok(())
    } else {
        let msg = format!("{exit_status:?}-Err:{}\nOut:{}", String::from_utf8(process.stderr)?, String::from_utf8(process.stdout)?);
        Err(anyhow::Error::msg(msg))
    }
}
