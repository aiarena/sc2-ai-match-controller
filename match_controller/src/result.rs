use anyhow::Context;
use core::fmt;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use tracing::info;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MatchResult {
    #[serde(rename = "match")]
    pub match_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot1_avg_step_time: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot1_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot2_avg_step_time: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot2_tags: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub result: ResultCode,
    pub game_steps: u32,
}

impl MatchResult {
    pub fn new_initialization_error(match_id: u32) -> Self {
        MatchResult {
            match_id,
            bot1_avg_step_time: None,
            bot1_tags: None,
            bot2_avg_step_time: None,
            bot2_tags: None,
            result: ResultCode::InitializationError,
            game_steps: 0,
        }
    }

    pub fn read_from_file() -> anyhow::Result<Self> {
        let file = File::open("/match/match_result.json").context("Failed to open match result file")?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).context("Failed to parse match result file")
    }

    pub fn write_to_file(&self) -> anyhow::Result<()> {
        let path = Path::new("/match/match_result.json");
        if path.exists() {
            // Keep the first result written. The game controller may attempt to overwrite
            // it (e.g. on a crash or retry), but the initial result is the authoritative one.
            let record = Self::read_from_file()?;
            info!("Match result already stored: {:?}", record);
            info!("Ignoring new match result: {:?}", self);
            return Ok(());
        }
        let file = File::create(path).context("Failed to create match result file")?;
        serde_json::to_writer_pretty(file, &self).context("Failed to write match result file")
    }

    pub fn delete_file() -> anyhow::Result<()> {
        let path = Path::new("/match/match_result.json");
        if path.exists() {
            std::fs::remove_file(path).context("Failed to delete match result file")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
pub enum ResultCode {
    Player1Crash,
    Player2Crash,
    Player1TimeOut,
    Player2TimeOut,
    Player1Win,
    Player2Win,
    Tie,
    InitializationError,
    Error,
}

impl Display for ResultCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
