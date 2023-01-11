use crate::matches::{Match, MatchPlayer};
use common::configuration::ac_config::ACConfig;
use common::models::bot_controller::PlayerNum;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameConfig {
    pub map: String,
    pub max_game_time: u32,
    pub max_frame_time: i32,
    pub timeout_secs: u64,
    pub strikes: i32,
    pub replay_path: String,
    pub match_id: u32,
    pub replay_name: String,
    pub disable_debug: bool,
    pub real_time: bool,
    pub visualize: bool,
    pub validate_race: bool,
    pub players: HashMap<PlayerNum, MatchPlayer>,
}

impl GameConfig {
    /// New default configuration
    pub fn new(m: &Match, ac_config: &ACConfig) -> Self {
        Self {
            map: m.map_name.clone(),
            max_game_time: ac_config.max_game_time,
            max_frame_time: ac_config.max_frame_time,
            timeout_secs: ac_config.timeout_secs,
            strikes: ac_config.strikes,
            replay_path: ac_config.replays_directory.clone(),
            match_id: m.match_id,
            replay_name: format!(
                "{}_{}_vs_{}.SC2Replay",
                m.match_id,
                &m.players[&PlayerNum::One].name,
                &m.players[&PlayerNum::Two].name
            ),
            disable_debug: ac_config.disable_debug,
            real_time: ac_config.realtime,
            visualize: ac_config.visualize,
            validate_race: ac_config.validate_race,
            players: m.players.clone(),
        }
    }

    pub fn player_1(&self) -> &MatchPlayer {
        &self.players[&PlayerNum::One]
    }
    pub fn player_2(&self) -> &MatchPlayer {
        &self.players[&PlayerNum::Two]
    }
    pub const fn map(&self) -> &String {
        &self.map
    }
    pub const fn disable_debug(&self) -> bool {
        self.disable_debug
    }
    pub const fn realtime(&self) -> bool {
        self.real_time
    }
    pub const fn max_game_time(&self) -> u32 {
        self.max_game_time
    }
    pub fn replay_path(&self) -> &str {
        &self.replay_path
    }
    pub const fn validate_race(&self) -> bool {
        self.validate_race
    }
}
