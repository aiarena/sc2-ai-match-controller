use bytes::Bytes;
use reqwest::Client;
use tracing::info;

use crate::settings::Settings;

pub async fn download_cache(settings: &Settings, url: &str, name: &str, etag: &str) -> anyhow::Result<Bytes> {
    let mut cache_url = url::Url::parse(&settings.caching_server_url).unwrap();
    cache_url = cache_url.join("/download").unwrap();
    let body = serde_json::json!({
        "uniqueKey": name,
        "url": url,
        "md5hash": etag,
    });
    let start = std::time::Instant::now();

    let response = match Client::new().post(cache_url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            info!("[http] failure download cache 0.000 MB in {:.3}s attempt 1", start.elapsed().as_secs_f64());
            return Err(anyhow::Error::from(e));
        }
    };
    let status = response.status();
    if !status.is_success() {
        let label = if status == reqwest::StatusCode::NOT_FOUND { "miss" } else { "failure" };
        info!("[http] {} download cache 0.000 MB in {:.3}s attempt 1", label, start.elapsed().as_secs_f64());
        return Err(anyhow::anyhow!("Cache download failed: {}", status));
    }
    let bytes = response.bytes().await.map_err(anyhow::Error::from)?;
    info!(
        "[http] success download cache {:.3} MB in {:.3}s attempt 1",
        bytes.len() as f64 / 1_000_000.0,
        start.elapsed().as_secs_f64()
    );
    Ok(bytes)
}
