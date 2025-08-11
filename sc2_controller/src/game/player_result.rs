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
impl PlayerResult {
    pub fn set_game_loops(&mut self, game_loops: u32) {
        self.game_loops = game_loops;
    }
    pub fn set_frame_time(&mut self, frame_time: f32) {
        self.frame_time = frame_time;
    }
    pub fn set_player_id(&mut self, player_id: u32) {
        self.player_id = player_id;
    }
    pub fn set_tags(&mut self, tags: indexmap::IndexSet<String>) {
        self.tags = tags;
    }
    pub fn set_result(&mut self, result: Sc2Result) {
        self.result = result;
    }
}
