use crate::game::player_result::PlayerResult;
use common::models::aiarena::aiarena_result::AiArenaResult;
use common::PlayerNum;

#[derive(Debug, Clone)]
pub struct GameResult {
    pub match_id: u32,
    pub player1_result: Option<PlayerResult>,
    pub player2_result: Option<PlayerResult>,
    pub result: Option<AiArenaResult>,
}

impl GameResult {
    pub const fn new(match_id: u32) -> Self {
        Self {
            match_id,
            player1_result: None,
            player2_result: None,
            result: None,
        }
    }
    pub fn has_any_result(&self) -> bool {
        self.player1_result.is_some() || self.player2_result.is_some()
    }
    pub fn set_error(&mut self) {
        self.result = Some(AiArenaResult::Error);
    }
    pub fn set_init_error(&mut self) {
        self.result = Some(AiArenaResult::InitializationError);
    }

    pub fn add_player_result(&mut self, player_num: PlayerNum, player_result: PlayerResult) {
        match player_num {
            PlayerNum::One => {
                self.player1_result = Some(player_result);
            }
            PlayerNum::Two => {
                self.player2_result = Some(player_result);
            }
        }
    }
}
