use serde::{Deserialize, Serialize};

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
