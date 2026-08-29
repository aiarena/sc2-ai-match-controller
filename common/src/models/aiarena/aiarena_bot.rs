use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaBot {
    pub name: String,
    pub game_display_id: String,
    pub bot_zip_url: String,
    pub bot_data_url: Option<String>,
    pub plays_race: String,
    #[serde(rename = "type")]
    pub _type: String,
    #[serde(default)]
    pub bot_base: Option<String>,
}
