use reqwest::Client;
use tracing::info;

use crate::settings::Settings;

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
