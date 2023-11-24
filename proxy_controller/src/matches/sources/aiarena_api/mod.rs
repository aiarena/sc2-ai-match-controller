use crate::matches::sources::file_source::errors::SubmissionError;
use crate::matches::sources::{LogsAndReplays, MatchSource};
use crate::matches::Match;
use async_trait::async_trait;
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use common::api::api_reference::aiarena::errors::AiArenaApiError;
use common::api::api_reference::aiarena::AiArenaResultForm;
use common::api::api_reference::ApiError;
use common::configuration::ac_config::ACConfig;
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::AiArenaMatch;
use common::paths::base_dir;
use common::PlayerNum;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tracing::log::error;
use tracing::{debug, info};

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
    async fn download_map(&self, ai_match: &AiArenaMatch, add_auth_header: bool) -> Result<(), ApiError<AiArenaApiError>> {
        let map_url = &ai_match.map.file;
        let map_name = &ai_match.map.name;
        info!("Downloading map {}", map_name);
        let map_bytes = self.api.download_map(map_url, add_auth_header).await?;
        let map_path = base_dir().join("maps").join(format!("{map_name}.SC2Map"));
        let mut file = tokio::fs::File::create(map_path).await?;
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
            let form = AiArenaResultForm::from(game_result)
                .add_bot_data(PlayerNum::One, &bot1_dir.join("data.zip"))
                .await
                .add_bot_data(PlayerNum::Two, &bot2_dir.join("data.zip"))
                .await
                .add_bot_log(PlayerNum::One, &bot1_dir.join("logs.zip"))
                .await
                .add_bot_log(PlayerNum::Two, &bot2_dir.join("logs.zip"))
                .await
                .add_replay(&replay_file)
                .await
                .add_arenaclient_logs(&arenaclient_log)
                .await;

            info!("{:?}", game_result);
            let status_result = self.api.submit_result(form.to_inner()).await;

            if status_result.is_err()
                || (status_result.as_ref().unwrap().is_client_error()
                    || status_result.as_ref().unwrap().is_server_error())
            {
                debug!("Error while submitting result. Sleeping...");
                error!("{:?}", status_result);
                attempt += 1;
                tokio::time::sleep(Duration::from_secs(10)).await;
            } else {
                break;
            }
        }
        Ok(())
    }
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
