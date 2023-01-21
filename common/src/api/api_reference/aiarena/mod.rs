use serde::{Deserialize, Serialize};
pub mod aiarena_api_client;
pub mod errors;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaMap {
    pub id: u32,
    pub name: String,
    pub file: String,
    pub enabled: bool,
    pub game_mode: i64,
    pub competitions: Vec<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaBot {
    pub id: u32,
    pub name: String,
    pub game_display_id: String,
    pub bot_zip: String,
    pub bot_zip_md5hash: String,
    pub bot_data: Option<String>,
    pub bot_data_md5hash: Option<String>,
    pub plays_race: String,
    #[serde(rename = "type")]
    pub _type: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaMatch {
    pub id: u32,
    pub bot1: AiArenaBot,
    pub bot2: AiArenaBot,
    pub map: AiArenaMap,
}
