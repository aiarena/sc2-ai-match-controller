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
