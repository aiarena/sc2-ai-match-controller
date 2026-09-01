use reqwest::Client;
use tracing::info;

use crate::settings::Settings;

pub async fn upload_cache(settings: &Settings, unique_key: &str, data: &[u8]) -> anyhow::Result<()> {
    let mut cache_url = url::Url::parse(&settings.caching_server_url).unwrap();
    cache_url = cache_url.join("/upload").unwrap();
    let size_mb = data.len() as f64 / 1_000_000.0;
    let part = reqwest::multipart::Part::bytes(data.to_vec()).file_name(unique_key.to_string());
    let form = reqwest::multipart::Form::new().part("file", part);
    let start = std::time::Instant::now();

    let response = match Client::new().post(cache_url).query(&[("uniqueKey", unique_key)]).multipart(form).send().await {
        Ok(r) => r,
        Err(e) => {
            info!("[http] failure upload cache {:.3} MB in {:.3}s attempt 1", size_mb, start.elapsed().as_secs_f64());
            return Err(anyhow::Error::from(e));
        }
    };
    match response.error_for_status() {
        Ok(_) => {
            info!("[http] success upload cache {:.3} MB in {:.3}s attempt 1", size_mb, start.elapsed().as_secs_f64());
            Ok(())
        }
        Err(e) => {
            info!("[http] failure upload cache {:.3} MB in {:.3}s attempt 1", size_mb, start.elapsed().as_secs_f64());
            Err(anyhow::Error::from(e))
        }
    }
}
