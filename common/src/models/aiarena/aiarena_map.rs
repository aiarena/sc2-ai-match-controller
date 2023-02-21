use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaMap {
    pub id: u32,
    pub name: String,
    pub file: String,
    pub enabled: bool,
    pub game_mode: i64,
    pub competitions: Vec<i64>,
}
