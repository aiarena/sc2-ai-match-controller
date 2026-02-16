use crate::PlayerNum;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StartBot {
    pub bot_name: String,
    pub bot_type: String,
    pub opponent_id: String,
    pub player_num: PlayerNum,
    pub match_id: u32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MapData {
    pub query: String,
    pub map_path: String,
}
