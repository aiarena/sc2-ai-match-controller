use crate::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use crate::api_reference::aiarena::errors::AiArenaApiError;
use crate::api_reference::aiarena::AiArenaMatch;
use crate::api_reference::ApiError;
use crate::matches::sources::file_source::errors::SubmissionError;
use crate::matches::sources::{AiArenaGameResult, LogsAndReplays, MatchSource};
use crate::matches::Match;
use common::async_trait::async_trait;
use common::configuration::ac_config::ACConfig;
use common::paths::base_dir;
use common::reqwest;
use common::tokio::io::AsyncWriteExt;
use common::tracing::log::error;
use common::tracing::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::time::Duration;

pub struct HttpApiSource {
    api: AiArenaApiClient,
}

impl HttpApiSource {
    pub fn new(settings: ACConfig) -> Result<Self, String> {
        let api_token = settings
            .api_token
            .as_ref()
            .ok_or_else(|| "Missing API Token".to_string())?;
        let api = AiArenaApiClient::new(&settings.base_website_url, api_token).map_err(|e| {
            format!(
                "URL ParseError on {:?}: {:?}",
                &settings.base_website_url, e
            )
        })?;
        Ok(Self { api })
    }
    async fn download_map(&self, ai_match: &AiArenaMatch) -> Result<(), ApiError<AiArenaApiError>> {
        let map_url = &ai_match.map.file;
        let map_name = &ai_match.map.name;
        info!("Downloading map {}", map_name);
        let map_bytes = self.api.download_map(map_url).await?;
        let map_path = base_dir().join("maps").join(format!("{}.SC2Map", map_name));
        let mut file = common::tokio::fs::File::create(map_path).await?;
        Ok(file.write_all(&map_bytes).await?)
    }
}

#[async_trait]
impl MatchSource for HttpApiSource {
    async fn has_next(&self) -> bool {
        return true;
    }

    async fn next_match(&self) -> Option<Match> {
        match self.api.get_match().await {
            Ok(m) => Some(Match::from(m)),
            Err(err) => {
                error!("{:?}", err);
                None
            }
        }
    }

    async fn submit_result(
        &self,
        game_result: &AiArenaGameResult,
        logs_and_replays: Option<LogsAndReplays>,
    ) -> Result<(), SubmissionError> {
        if logs_and_replays.is_none() {
            return Err(SubmissionError::LogsAndReplaysNull);
        }
        let LogsAndReplays {
            bot1_dir,
            bot2_dir,
            arenaclient_log,
            replay_file,
        } = logs_and_replays.unwrap();

        let mut attempt = 0;
        while attempt < 60 {
            debug!("Attempting to submit result. Attempt number: {}", attempt);
            let mut files = common::reqwest::multipart::Form::new()
                .text("match", game_result.match_id.to_string())
                .text("type", game_result.result.to_string())
                .text("game_steps", game_result.game_steps.to_string());
            if let Ok(part) = create_part_from_path(&bot1_dir.join("data.zip")).await {
                files = files.part("bot1_data", part)
            };

            if let Ok(part) = create_part_from_path(&bot2_dir.join("data.zip")).await {
                files = files.part("bot2_data", part)
            }

            if let Ok(part) = create_part_from_path(&bot1_dir.join("logs.zip")).await {
                files = files.part("bot1_log", part)
            }
            if let Ok(part) = create_part_from_path(&bot2_dir.join("logs.zip")).await {
                files = files.part("bot2_log", part)
            }
            if let Ok(part) = create_part_from_path(&arenaclient_log).await {
                files = files.part("arenaclient_log", part)
            }
            if let Ok(part) = create_part_from_path(&replay_file).await {
                files = files.part("replay_file", part)
            } else {
                println!("{:?}", &replay_file);
                error!("{:?}", create_part_from_path(&replay_file).await)
            }

            if let Some(bot1_avg_step_time) = game_result.bot1_avg_step_time {
                let avg_step_time = if bot1_avg_step_time.is_finite() {
                    bot1_avg_step_time
                } else {
                    0f32
                };
                files = files.text("bot1_avg_step_time", avg_step_time.to_string());
            }
            if let Some(bot2_avg_step_time) = game_result.bot2_avg_step_time {
                let avg_step_time = if bot2_avg_step_time.is_finite() {
                    bot2_avg_step_time
                } else {
                    0f32
                };
                files = files.text("bot2_avg_step_time", avg_step_time.to_string());
            }
            if let Some(bot1_tags) = &game_result.bot1_tags {
                files = files.text("bot1_tags", serde_json::to_string(&bot1_tags).unwrap());
            }
            if let Some(bot2_tags) = &game_result.bot2_tags {
                files = files.text("bot2_tags", serde_json::to_string(&bot2_tags).unwrap());
            }
            info!("{:?}", game_result);
            let status = self.api.submit_result(files).await;
            if status.is_client_error() || status.is_server_error() {
                debug!("Error while submitting result. Sleeping...");
                attempt += 1;
                common::tokio::time::sleep(Duration::from_secs(10)).await;
            } else {
                break;
            }
        }
        Ok(())
    }
}
async fn create_part_from_path(path: &Path) -> Result<reqwest::multipart::Part, std::io::Error> {
    let file_name = String::from(path.file_name().and_then(|p| p.to_str()).unwrap());
    let file = common::tokio::fs::read(path).await?;

    let file_part = reqwest::multipart::Part::bytes(file).file_name(file_name);
    Ok(file_part)
}
fn open_results_file(results_file_path: &str) -> Result<File, SubmissionError> {
    let results_file_path = std::path::Path::new(results_file_path);

    OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(results_file_path)
        .map_err(SubmissionError::FileOpen)
}

#[derive(Deserialize, Serialize, Default, Debug)]
struct Results {
    results: Vec<AiArenaGameResult>,
}

#[cfg(test)]
mod tests {}
