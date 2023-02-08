use crate::models::aiarena::aiarena_bot::AiArenaBot;
use crate::models::aiarena::aiarena_map::AiArenaMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaMatch {
    pub id: u32,
    pub bot1: AiArenaBot,
    pub bot2: AiArenaBot,
    pub map: AiArenaMap,
}
