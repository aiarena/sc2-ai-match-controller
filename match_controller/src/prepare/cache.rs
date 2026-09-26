use bytes::Bytes;
use reqwest::Client;
use tracing::info;

use crate::settings::Settings;

pub async fn download_cache(settings: &Settings, _url: &str, name: &str, etag: &str) -> anyhow::Result<Bytes> {
    let url = settings.cache_object_url(name, etag);
    let start = std::time::Instant::now();

    let response = match Client::new().get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            info!("[http] failure download cache {} 0.000 MB in {:.3}s attempt 1", name, start.elapsed().as_secs_f64());
            return Err(anyhow::Error::from(e));
        }
    };
    let status = response.status();
    if !status.is_success() {
        let label = if status == reqwest::StatusCode::NOT_FOUND { "miss" } else { "failure" };
        info!("[http] {} download cache {} 0.000 MB in {:.3}s attempt 1", label, name, start.elapsed().as_secs_f64());
        return Err(anyhow::anyhow!("Cache download failed: {}", status));
    }
    let bytes = response.bytes().await.map_err(anyhow::Error::from)?;
    info!("[http] success download cache {} {:.3} MB in {:.3}s attempt 1", name, bytes.len() as f64 / 1_000_000.0, start.elapsed().as_secs_f64());
    Ok(bytes)
}

pub async fn upload_cache(settings: &Settings, name: &str, etag: &str, data: &[u8]) -> anyhow::Result<()> {
    let size_mb = data.len() as f64 / 1_000_000.0;
    let url = settings.cache_object_url(name, etag);
    let start = std::time::Instant::now();

    let response = match Client::new().put(&url).body(data.to_vec()).send().await {
        Ok(r) => r,
        Err(e) => {
            info!("[http] failure upload cache {} {:.3} MB in {:.3}s attempt 1", name, size_mb, start.elapsed().as_secs_f64());
            return Err(anyhow::Error::from(e));
        }
    };
    match response.error_for_status() {
        Ok(_) => {
            info!("[http] success upload cache {} {:.3} MB in {:.3}s attempt 1", name, size_mb, start.elapsed().as_secs_f64());
            Ok(())
        }
        Err(e) => {
            info!("[http] failure upload cache {} {:.3} MB in {:.3}s attempt 1", name, size_mb, start.elapsed().as_secs_f64());
            Err(anyhow::Error::from(e))
        }
    }
}
