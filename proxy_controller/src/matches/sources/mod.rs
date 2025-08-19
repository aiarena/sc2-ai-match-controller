use async_trait::async_trait;
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::Match;
use std::path::PathBuf;

pub mod aiarena_api;
pub mod file_source;
pub mod test_source;

use crate::matches::sources::file_source::errors::SubmissionError;
pub use file_source::FileSource;

#[async_trait]
pub trait MatchSource: Sync + Send {
    async fn has_next(&self) -> bool;
    async fn next_match(&self) -> Option<Match>;
    async fn submit_result(
        &self,
        game_result: &AiArenaGameResult,
        logs_and_replays: Option<LogsAndReplays>,
    ) -> Result<(), SubmissionError>;
}

#[async_trait]
impl<T: MatchSource + ?Sized> MatchSource for Box<T> {
    async fn has_next(&self) -> bool {
        (**self).has_next().await
    }

    async fn next_match(&self) -> Option<Match> {
        (**self).next_match().await
    }

    async fn submit_result(
        &self,
        game_result: &AiArenaGameResult,
        logs_and_replays: Option<LogsAndReplays>,
    ) -> Result<(), SubmissionError> {
        (**self).submit_result(game_result, logs_and_replays).await
    }
}

#[derive(Clone, Debug)]
pub struct LogsAndReplays {
    pub upload_url: String,
    pub bot1_name: String,
    pub bot2_name: String,
    pub bot1_dir: PathBuf,
    pub bot2_dir: PathBuf,
    pub arenaclient_log: PathBuf,
    pub replay_file: PathBuf,
}
