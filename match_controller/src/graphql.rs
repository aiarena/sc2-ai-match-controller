use anyhow::{anyhow, Context};
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tracing::{error, info};

use crate::settings::Settings;

#[derive(Debug, Deserialize)]
pub struct GraphQLFieldError {
    pub field: String,
    pub messages: Vec<String>,
}

pub async fn post_graphql(settings: &Settings, operation: &str, body: serde_json::Value) -> anyhow::Result<String> {
    let graphql_url = format!("{}/graphql/", settings.base_website_url.trim_end_matches('/'));
    let mut last_err = None;
    for attempt in 1..=10 {
        let start = std::time::Instant::now();
        let response = match Client::new()
            .post(&graphql_url)
            .header("Authorization", format!("Token {}", &settings.api_token))
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                info!("[http] failure {} graph 0.000 MB in {:.3}s attempt {}", operation, start.elapsed().as_secs_f64(), attempt);
                error!("GraphQL attempt {}/10 failed: {}", attempt, e);
                last_err = Some(anyhow::Error::from(e));
                if attempt < 10 {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
                continue;
            }
        };
        let status = response.status();
        if status.is_server_error() {
            info!("[http] failure {} graph 0.000 MB in {:.3}s attempt {}", operation, start.elapsed().as_secs_f64(), attempt);
            error!("GraphQL attempt {}/10 server error {}: retrying", attempt, status);
            last_err = Some(anyhow!("GraphQL server error: {}", status));
            if attempt < 10 {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
            continue;
        }
        if !status.is_success() {
            info!("[http] failure {} graph 0.000 MB in {:.3}s attempt {}", operation, start.elapsed().as_secs_f64(), attempt);
            return Err(anyhow!("GraphQL request failed: {}", status));
        }
        let text = response.text().await.context("Failed to read GraphQL response body")?;
        info!(
            "[http] success {} graph {:.3} MB in {:.3}s attempt {}",
            operation,
            text.len() as f64 / 1_000_000.0,
            start.elapsed().as_secs_f64(),
            attempt
        );
        return Ok(text);
    }
    Err(last_err.unwrap())
}
