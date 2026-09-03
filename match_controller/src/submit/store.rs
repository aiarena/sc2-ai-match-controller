use anyhow::Context;
use reqwest::Client;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use tracing::{error, info};

use crate::settings::Settings;

pub async fn upload_file(settings: &Settings, name: &str, file_path: &Path) -> anyhow::Result<String> {
    if !file_path.exists() {
        return Ok(String::new());
    }
    let data = tokio::fs::read(file_path).await.with_context(|| format!("Failed to read file: {}", file_path.display()))?;
    upload_data(settings, name, &data).await
}

pub async fn upload_zip(settings: &Settings, name: &str, directory: &Path, cacheable: bool) -> anyhow::Result<String> {
    if !directory.exists() {
        return Ok(String::new());
    }
    let tmp = tempfile::NamedTempFile::new()?;
    zip_directory(tmp.path(), directory).with_context(|| format!("Failed to zip: {}", directory.display()))?;
    let data = tokio::fs::read(tmp.path()).await?;
    if cacheable {
        if let Err(e) = super::cache::upload_cache(settings, name, &data).await {
            info!("Cache upload failed: {}", e);
        }
    }
    upload_data(settings, name, &data).await
}

async fn upload_data(settings: &Settings, name: &str, data: &[u8]) -> anyhow::Result<String> {
    let size_mb = data.len() as f64 / 1_000_000.0;
    let mut last_err = None;
    for attempt in 1..=10 {
        let (upload_id, upload_url) = super::upload_url::request_upload_url(settings).await?;
        info!("Uploading {:.3} MB -> {}", size_mb, upload_id);
        let start = std::time::Instant::now();
        let response = match Client::new().put(&upload_url).body(data.to_vec()).send().await {
            Ok(r) => r,
            Err(e) => {
                info!("[http] failure upload store {} {:.3} MB in {:.3}s attempt {}", name, size_mb, start.elapsed().as_secs_f64(), attempt);
                error!("Upload attempt {}/10 failed: {}", attempt, e);
                last_err = Some(anyhow::anyhow!("{}", e));
                if attempt < 10 {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                continue;
            }
        };
        let status = response.status();
        let elapsed = start.elapsed().as_secs_f64();

        if status.is_success() {
            info!("[http] success upload store {} {:.3} MB in {:.3}s attempt {}", name, size_mb, elapsed, attempt);
            return Ok(upload_id);
        }
        info!("[http] failure upload store {} {:.3} MB in {:.3}s attempt {}", name, size_mb, elapsed, attempt);
        if !status.is_server_error() {
            return Err(anyhow::anyhow!("Upload to store failed: {}", status));
        }
        error!("Upload attempt {}/10 server error {}: retrying", attempt, status);
        last_err = Some(anyhow::anyhow!("Upload to store failed: {}", status));
        if attempt < 10 {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
    Err(last_err.unwrap())
}

pub(super) fn zip_directory(zip_path: &Path, directory: &Path) -> anyhow::Result<()> {
    // Remove the empty placeholder file so 7z creates the archive from scratch.
    std::fs::remove_file(zip_path).ok();
    let file = zip_path.to_string_lossy().to_string();
    let dir = directory.join("*").to_string_lossy().to_string();
    let process = Command::new("7z").arg("a").arg("-tzip").arg(&file).arg(&dir).arg("-r").arg("-y").output()?;
    if process.status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Error while zipping: {:?}", String::from_utf8(process.stderr)))
    }
}
