use serde::{Deserialize, Serialize};

use crate::game::sc2_result::Sc2Result;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerResult {
    /// Game loops
    pub game_loops: u32,
    /// Frame time
    pub frame_time: f32,
    /// Player id
    pub player_id: u32,
    /// Tags
    #[serde(skip_serializing_if = "indexmap::IndexSet::is_empty")]
    pub tags: indexmap::IndexSet<String>,
    /// Result
    pub result: Sc2Result,
}
