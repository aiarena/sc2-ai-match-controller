use common::models::aiarena::aiarena_match::{MatchPlayer, MatchRequest};
use common::models::aiarena::bot_race::BotRace;
use common::PlayerNum;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameConfig {
    pub map: String,
    pub max_game_time: u32,
    pub max_frame_time: i32,
    pub timeout_secs: u64,
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
    pub fn from_file(match_request: &MatchRequest) -> Self {
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
                    bot_type: "linux".to_string(), // Bot type is irrelevant for the game controller
                    bot_base: "".to_string(),      // Bot base is irrelevant for the game controller
                },
            ),
            (
                PlayerNum::Two,
                MatchPlayer {
                    id: player_2_name.to_string(),
                    name: player_2_name.to_string(),
                    race: BotRace::from_str(&match_request.player_2_race.to_string()),
                    bot_type: "linux".to_string(), // Bot type is irrelevant for the game controller
                    bot_base: "".to_string(),      // Bot base is irrelevant for the game controller
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

            max_game_time: 60486,
            max_frame_time: 40,
            timeout_secs: 30,
            replay_path: "/root/StarCraftII/maps".to_string(),
            replay_name: replay_name,
            disable_debug: true,
            real_time: false,
            validate_race: true,
            visualize: false, // Not used
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
