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
                    .map_err(ConfigError::Foreign)
            })
    }
}

pub async fn get_config_from_match_controller(
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

/// Constructs the URL for a component of the arena client - bot, match, or sc2 controller.
pub fn get_host_url(prefix: &str, default_port: Port) -> String {
    let host = std::env::var(format!("{prefix}_HOST")).unwrap_or_else(|_| "0.0.0.0".into());
    let port = std::env::var(format!("{prefix}_PORT")).unwrap_or_else(|_| default_port.to_string());
    format!("{host}:{port}")
}

/// Constructs the URL for the match component of the arena client.
/// This is used to retrieve the configuration of the arena client.
pub fn get_match_controller_url_from_env(prefix: &str) -> String {
    let host = std::env::var(format!("{prefix}_MATCH_CONTROLLER_HOST"))
        .unwrap_or_else(|_| "127.0.0.1".into());
    let port =
        std::env::var(format!("{prefix}_MATCH_CONTROLLER_PORT")).unwrap_or_else(|_| "8080".into());
    format!("{host}:{port}")
}

/// Used by bot controller to get the host for the game.
pub fn get_game_host(prefix: &str) -> String {
    std::env::var(format!("{prefix}_GAME_HOST")).unwrap_or_else(|_| "127.0.0.1".into())
}

/// Used by bot controller to get the port for the game.
pub fn get_game_port(prefix: &str) -> String {
    std::env::var(format!("{prefix}_GAME_PORT")).unwrap_or_else(|_| "8083".into())
}
