use crate::matches::sources::file_source::errors::SubmissionError;
use crate::matches::sources::{LogsAndReplays, MatchSource};
use crate::matches::Match;
use async_trait::async_trait;
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use common::api::api_reference::aiarena::errors::AiArenaApiError;
use common::api::api_reference::aiarena::{create_part_from_bytes, AiArenaResultForm};
use common::api::api_reference::ApiError;
use common::configuration::ac_config::ACConfig;
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::AiArenaMatch;
use common::paths::base_dir;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
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
    async fn download_map(
        &self,
        ai_match: &AiArenaMatch,
        add_auth_header: bool,
    ) -> Result<(), ApiError<AiArenaApiError>> {
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
            upload_url,
            bot1_name,
            bot2_name,
            bot1_dir,
            bot2_dir,
            arenaclient_log,
            replay_file,
        } = logs_and_replays.unwrap();

        let mut attempt = 0;

        let bot1_data = get_file_and_filename(&bot1_dir.join("data.zip")).await;
        let bot2_data = get_file_and_filename(&bot2_dir.join("data.zip")).await;

        let bot1_log = get_file_and_filename(&bot1_dir.join("logs.zip")).await;
        let bot2_log = get_file_and_filename(&bot2_dir.join("logs.zip")).await;

        let replay = get_file_and_filename(&replay_file).await;
        let arenaclient_logs = get_file_and_filename(&arenaclient_log).await;

        if let Ok(ref x) = bot1_data {
            if let Err(e) = self
                .api
                .cache_upload(&upload_url, format!("{}_data", bot1_name), &x.0)
                .await
            {
                error!("Error uploading to cache server: {}", e);
            }
        }
        if let Ok(ref x) = bot2_data {
            if let Err(e) = self
                .api
                .cache_upload(&upload_url, format!("{}_data", bot2_name), &x.0)
                .await
            {
                error!("Error uploading to cache server: {}", e);
            }
        }
        while attempt < 60 {
            debug!("Attempting to submit result. Attempt number: {}", attempt);

            let mut form = AiArenaResultForm::from(game_result).to_inner();
            if let Ok(ref x) = bot1_data {
                form = form.part(
                    "bot1_data",
                    create_part_from_bytes(x.0.clone(), x.1.clone()),
                );
            }
            if let Ok(ref x) = bot2_data {
                form = form.part(
                    "bot2_data",
                    create_part_from_bytes(x.0.clone(), x.1.clone()),
                );
            }
            if let Ok(ref x) = bot1_log {
                form = form.part("bot1_log", create_part_from_bytes(x.0.clone(), x.1.clone()));
            }
            if let Ok(ref x) = bot2_log {
                form = form.part("bot2_log", create_part_from_bytes(x.0.clone(), x.1.clone()));
            }
            if let Ok(ref x) = replay {
                form = form.part(
                    "replay_file",
                    create_part_from_bytes(x.0.clone(), x.1.clone()),
                );
            }
            if let Ok(ref x) = arenaclient_logs {
                form = form.part(
                    "arenaclient_log",
                    create_part_from_bytes(x.0.clone(), x.1.clone()),
                );
            }

            info!("{:?}", game_result);
            let status_result = self.api.submit_result(form).await;

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

pub async fn get_file_and_filename(path: &PathBuf) -> Result<(Vec<u8>, String), std::io::Error> {
    let file_name = String::from(path.file_name().and_then(|p| p.to_str()).unwrap());
    let file = tokio::fs::read(path).await?;

    Ok((file, file_name))
}
