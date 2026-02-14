use crate::models::aiarena::aiarena_result::AiArenaResult;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiArenaGameResult {
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
    pub result: AiArenaResult,
    pub game_steps: u32,
}

use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

impl AiArenaGameResult {
    // Create an instance of InitializationError
    pub fn new_initialization_error(match_id: u32) -> Self {
        AiArenaGameResult {
            match_id,
            bot1_avg_step_time: None,
            bot1_tags: None,
            bot2_avg_step_time: None,
            bot2_tags: None,
            result: AiArenaResult::InitializationError,
            game_steps: 0,
        }
    }

    // Reads AiArenaGameResult from disk.
    pub fn from_json_file() -> Result<Self, Box<dyn Error>> {
        let file = File::open("/logs/sc2_controller/match_result.json")?;
        let reader = BufReader::new(file);
        let result = serde_json::from_reader(reader)?;
        Ok(result)
    }

    // Writes the AiArenaGameResult instance to disk.
    pub fn to_json_file(&self) -> Result<(), Box<dyn Error>> {
        let path = Path::new("/logs/sc2_controller/match_result.json");

        // If a valid match result is already stored, keep it
        if path.exists() {
            let record = Self::from_json_file()?;
            println!("Match result already stored: {:?}", record);
            println!("Ignoring new match result: {:?}", self);
            return Ok(());
        }

        // Otherwise, store this match result
        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self)?;
        Ok(())
    }

    // Deletes the match result file from disk.
    pub fn delete_json_file() -> Result<(), Box<dyn Error>> {
        let path = Path::new("/logs/sc2_controller/match_result.json");
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
