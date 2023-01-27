pub mod ac_config;

use crate::configuration::ac_config::ACConfig;
use crate::utilities::portpicker::Port;
use async_trait::async_trait;
use config::{AsyncSource, Config, ConfigError, FileFormat, Format, Map};
use std::fmt::Debug;
use std::time::Duration;

#[derive(Debug)]
pub struct HttpSource<F: Format> {
    pub config_uri: String,
    pub health_uri: String,
    pub format: F,
}

#[async_trait]
impl<F: Format + Send + Sync + Debug> AsyncSource for HttpSource<F> {
    async fn collect(&self) -> Result<Map<String, config::Value>, ConfigError> {
        let max_retries = 60; //180 seconds
        let mut retries = 0;
        while (reqwest::get(&self.health_uri).await).is_err() {
            if retries > max_retries {
                break;
            }

            retries += 1;

            tokio::time::sleep(Duration::from_secs(3)).await;
        }

        reqwest::get(&self.config_uri)
            .await
            .map_err(|e| ConfigError::Foreign(Box::new(e)))? // error conversion is possible from custom AsyncSource impls
            .text()
            .await
            .map_err(|e| ConfigError::Foreign(Box::new(e)))
            .and_then(|text| {
                self.format
                    .parse(Some(&self.config_uri), &text)
                    .map_err(|e| ConfigError::Foreign(e))
            })
    }
}

pub async fn get_config_from_proxy(
    config_uri: String,
    health_uri: String,
    prefix: &str,
) -> Result<ACConfig, ConfigError> {
    Config::builder()
        .add_async_source(HttpSource {
            config_uri,
            health_uri,
            format: FileFormat::Json,
        })
        .add_source(config::Environment::with_prefix(prefix))
        .build()
        .await?
        .try_deserialize::<ACConfig>()
}

pub fn get_host_url(prefix: &str, default_port: Port) -> String {
    let host = std::env::var(format!("{prefix}_HOST")).unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var(format!("{prefix}_PORT")).unwrap_or_else(|_| default_port.to_string());
    format!("{host}:{port}")
}

pub fn get_proxy_url_from_env(prefix: &str) -> String {
    format!("{}:{}", get_proxy_host(prefix), get_proxy_port(prefix))
}

pub fn get_proxy_host(prefix: &str) -> String {
    std::env::var(format!("{prefix}_PROXY_HOST")).unwrap_or_else(|_| "127.0.0.1".into())
}

pub fn get_proxy_port(prefix: &str) -> String {
    std::env::var(format!("{prefix}_PROXY_PORT")).unwrap_or_else(|_| "8080".into())
}
