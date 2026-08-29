pub mod graphql;

use crate::matches::sources::file_source::errors::SubmissionError;
use crate::matches::sources::{LogsAndReplays, MatchSource};
use async_trait::async_trait;
use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use common::api::api_reference::aiarena::errors::AiArenaApiError;
use common::api::api_reference::ApiError;
use common::configuration::ac_config::ACConfig;
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::{AiArenaMatch, Match};
use common::paths::base_dir;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use tracing::info;
use tracing::log::error;

pub struct HttpApiSource {
    api: AiArenaApiClient,
    website_url: String,
    token: String,
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
        Ok(Self {
            api,
            website_url: settings.base_website_url.clone(),
            token: api_token.clone(),
        })
    }

    async fn download_map(
        &self,
        ai_match: &AiArenaMatch,
        add_auth_header: bool,
    ) -> Result<(), ApiError<AiArenaApiError>> {
        let map_url = &ai_match.map.download_link;
        let map_name = &ai_match.map.name;
        info!("Downloading map {}", map_name);
        let map_bytes = self.api.download_map(map_url, add_auth_header).await?;
        let map_path = base_dir().join("maps").join(format!("{map_name}.SC2Map"));
        let mut file = tokio::fs::File::create(map_path).await?;
        Ok(file.write_all(&map_bytes).await?)
    }

    async fn upload_file(&self, path: &PathBuf) -> Result<String, SubmissionError> {
        if path.exists() {
            graphql::upload_file_with_retries(&self.website_url, &self.token, path, 60)
                .await
                .map_err(|e| {
                    error!("Failed to upload {}: {}", path.display(), e);
                    SubmissionError::LogsAndReplaysNull
                })
        } else {
            Ok(String::new())
        }
    }
}

#[async_trait]
impl MatchSource for HttpApiSource {
    async fn has_next(&self) -> bool {
        true
    }

    async fn next_match(&self) -> Option<Match> {
        match graphql::get_next_match(&self.website_url, &self.token).await {
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

        let bot1_data = get_file_and_filename(&bot1_dir.join("data.zip")).await;
        let bot2_data = get_file_and_filename(&bot2_dir.join("data.zip")).await;

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

        let replay_id = self.upload_file(&replay_file).await?;
        let arenaclient_log_id = self.upload_file(&arenaclient_log).await?;
        let bot1_data_id = self.upload_file(&bot1_dir.join("data.zip")).await?;
        let bot2_data_id = self.upload_file(&bot2_dir.join("data.zip")).await?;
        let bot1_log_id = self.upload_file(&bot1_dir.join("logs.zip")).await?;
        let bot2_log_id = self.upload_file(&bot2_dir.join("logs.zip")).await?;

        let input = graphql::SubmitResultInput {
            match_id: graphql::encode_match_id(&game_result.match_id.to_string()),
            result_type: game_result.result.to_string(),
            game_steps: game_result.game_steps,
            bot1_avg_step_time: game_result.bot1_avg_step_time.unwrap_or(0.0),
            bot2_avg_step_time: game_result.bot2_avg_step_time.unwrap_or(0.0),
            bot1_tags: game_result.bot1_tags.clone().unwrap_or_default(),
            bot2_tags: game_result.bot2_tags.clone().unwrap_or_default(),
            replay_file: replay_id,
            arenaclient_log: arenaclient_log_id,
            bot1_data: bot1_data_id,
            bot2_data: bot2_data_id,
            bot1_log: bot1_log_id,
            bot2_log: bot2_log_id,
        };

        graphql::submit_result_with_retries(&self.website_url, &self.token, &input, 60)
            .await
            .map_err(|e| {
                error!("Failed to submit result via GraphQL: {}", e);
                SubmissionError::LogsAndReplaysNull
            })?;

        Ok(())
    }
}

async fn get_file_and_filename(path: &PathBuf) -> Result<(Vec<u8>, String), std::io::Error> {
    let file_name = String::from(path.file_name().and_then(|p| p.to_str()).unwrap());
    let file = tokio::fs::read(path).await?;
    Ok((file, file_name))
}
