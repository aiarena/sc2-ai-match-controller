use common::configuration::ac_config::ACConfig;
use common::models::aiarena::aiarena_match::{Match, MatchPlayer, MatchRequest};
use common::models::aiarena::bot_race::BotRace;
use common::models::bot_controller::BotType;
use common::PlayerNum;
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
    // In this iteration the configuration is read from a file stored with the start request.
    // In the next iteration, the configuration will be read from the environment variables.
    pub fn from_file(ac_config: &ACConfig, match_request: &MatchRequest) -> Self {
        let match_id = match_request.match_id;
        let map_name = match_request.map_name.clone();
        let player_1_name = match_request.player_1_name.clone();
        let player_2_name = match_request.player_2_name.clone();

        let players = HashMap::from([
            (
                PlayerNum::One,
                MatchPlayer {
                    id: player_1_name.to_string(),
                    name: player_1_name.to_string(),
                    race: BotRace::from_str(&match_request.player_1_race.to_string()),
                    bot_type: BotType::Python, // Bot type is irrelevant for the game controller
                },
            ),
            (
                PlayerNum::Two,
                MatchPlayer {
                    id: player_2_name.to_string(),
                    name: player_2_name.to_string(),
                    race: BotRace::from_str(&match_request.player_2_race.to_string()),
                    bot_type: BotType::Python, // Bot type is irrelevant for the game controller
                },
            ),
        ]);
        let replay_name = format!(
            "{}_{}_vs_{}.SC2Replay",
            match_id, player_1_name, player_2_name
        );

        Self {
            match_id: match_id,
            map: map_name.to_string(),
            players: players,

            max_game_time: ac_config.max_game_time,
            max_frame_time: ac_config.max_frame_time,
            timeout_secs: ac_config.timeout_secs,
            strikes: ac_config.strikes,
            replay_path: "/root/StarCraftII/maps".to_string(),
            replay_name: replay_name,
            disable_debug: ac_config.disable_debug,
            real_time: ac_config.realtime,
            visualize: ac_config.visualize,
            validate_race: ac_config.validate_race,
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
