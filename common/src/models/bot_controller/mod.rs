use crate::PlayerNum;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StartBot {
    pub bot_name: String,
    pub bot_type: String,
    pub opponent_id: String,
    pub player_num: PlayerNum,
    pub match_id: u32,
}

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Clone)]
pub struct MapData {
    pub query: String,
    pub map_path: String,
}
