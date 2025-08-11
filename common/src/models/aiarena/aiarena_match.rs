use crate::models::aiarena::aiarena_bot::AiArenaBot;
use crate::models::aiarena::aiarena_map::AiArenaMap;
use crate::models::aiarena::bot_race::BotRace;
use crate::models::bot_controller::BotType;
use crate::PlayerNum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AiArenaMatch {
    pub id: u32,
    pub bot1: AiArenaBot,
    pub bot2: AiArenaBot,
    pub map: AiArenaMap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchPlayer {
    pub id: String,
    pub name: String,
    pub race: BotRace,
    pub bot_type: BotType,
}

impl MatchPlayer {
    pub fn from_aiarena_match(player_num: PlayerNum, ai_match: &AiArenaMatch) -> Self {
        match player_num {
            PlayerNum::One => Self {
                id: ai_match.bot1.game_display_id.clone(),
                name: ai_match.bot1.name.clone(),
                race: BotRace::from_str(&ai_match.bot1.plays_race),
                bot_type: BotType::from_str(&ai_match.bot1._type).unwrap(),
            },
            PlayerNum::Two => Self {
                id: ai_match.bot2.game_display_id.clone(),
                name: ai_match.bot2.name.clone(),
                race: BotRace::from_str(&ai_match.bot2.plays_race),
                bot_type: BotType::from_str(&ai_match.bot2._type).unwrap(),
            },
        }
    }

    pub fn from_file_source(bot_line: &[String]) -> Result<Self, SerializationError> {
        Ok(Self {
            id: bot_line
                .get(0)
                .ok_or_else(|| SerializationError::ParsingError)?
                .to_string(),
            name: bot_line
                .get(1)
                .ok_or_else(|| SerializationError::ParsingError)?
                .to_string(),
            race: BotRace::from_str(
                bot_line
                    .get(2)
                    .ok_or_else(|| SerializationError::ParsingError)?,
            ),
            bot_type: BotType::from_str(
                bot_line
                    .get(3)
                    .ok_or_else(|| SerializationError::ParsingError)?,
            )
            .map_err(|_| SerializationError::ParsingError)?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Match {
    pub match_id: u32,
    pub players: HashMap<PlayerNum, MatchPlayer>,
    pub map_name: String,
    pub aiarena_match: Option<AiArenaMatch>,
}

impl From<AiArenaMatch> for Match {
    fn from(ai_match: AiArenaMatch) -> Self {
        let mut players = HashMap::with_capacity(2);
        players.insert(
            PlayerNum::One,
            MatchPlayer::from_aiarena_match(PlayerNum::One, &ai_match),
        );

        players.insert(
            PlayerNum::Two,
            MatchPlayer::from_aiarena_match(PlayerNum::Two, &ai_match),
        );

        Self {
            match_id: ai_match.id,
            players,
            map_name: ai_match.map.name.clone(),
            aiarena_match: Some(ai_match),
        }
    }
}

#[derive(Debug, Clone)]
pub enum SerializationError {
    ParsingError,
}

impl std::fmt::Display for SerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializationError::ParsingError => write!(f, "Error parsing file"),
        }
    }
}

impl std::error::Error for SerializationError {}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct MatchRequest {
    pub match_id: u32,

    pub player_1_id: String,
    pub player_1_name: String,

    pub player_2_id: String,
    pub player_2_name: String,

    // These are SC2-specific. Will be abstracted in the future.
    pub map_name: String,
    pub player_1_race: u8,
    pub player_2_race: u8,
}

impl From<Match> for MatchRequest {
    fn from(a_match: Match) -> Self {
        Self {
            match_id: a_match.match_id,
            player_1_id: a_match.players[&PlayerNum::One].id.clone(),
            player_1_name: a_match.players[&PlayerNum::One].name.clone(),
            player_2_id: a_match.players[&PlayerNum::Two].id.clone(),
            player_2_name: a_match.players[&PlayerNum::Two].name.clone(),
            map_name: a_match.map_name.clone(),
            player_1_race: a_match.players[&PlayerNum::One].race as u8,
            player_2_race: a_match.players[&PlayerNum::Two].race as u8,
        }
    }
}

impl MatchRequest {
    pub fn read() -> Self {
        config::Config::builder()
            .add_source(
                config::File::new(
                    "/logs/sc2_controller/match-request.toml",
                    config::FileFormat::Toml,
                )
                .required(false),
            )
            .add_source(config::Environment::default())
            .build()
            .expect("Could not read match request data")
            .try_deserialize::<MatchRequest>()
            .expect("Could not parse match request data")
    }

    pub fn write(&self) -> Result<(), std::io::Error> {
        let dir_path = "/logs/sc2_controller";
        let toml_str = toml::to_string(self).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not serialize match request data",
            )
        })?;
        tracing::debug!("Writing match request to file: {}", toml_str);

        std::fs::create_dir_all(dir_path)?;
        std::fs::write("/logs/sc2_controller/match-request.toml", toml_str)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct PlayerInfo {
    pub num: PlayerNum,
    pub id: String,
    pub name: String,

    // This is SC2-specific. Will be abstracted in the future.
    pub race: u8,
}

impl PlayerInfo {
    /// Reads player information for the player with the given client port.
    pub fn read(port: u16) -> Option<Self> {
        let file_path = format!("/logs/sc2_controller/player-{}.toml", port);

        // If file does not exist, return None
        if !std::path::Path::new(&file_path).exists() {
            return None;
        }

        Some(
            config::Config::builder()
                .add_source(config::File::new(&file_path, config::FileFormat::Toml).required(false))
                .add_source(config::Environment::default())
                .build()
                .expect("Could not read player information")
                .try_deserialize::<PlayerInfo>()
                .expect("Could not parse player information"),
        )
    }

    /// Writes player information for the player with the given port.
    pub fn write(&self, port: u16) -> Result<(), std::io::Error> {
        let dir_path = "/logs/sc2_controller";
        let toml_str = toml::to_string(self).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not serialize player information",
            )
        })?;
        tracing::debug!(
            "Writing player information to {}:\n{}",
            format!("{}/player-{}.toml", dir_path, port),
            &toml_str
        );

        std::fs::create_dir_all(dir_path)?;
        std::fs::write(format!("{}/player-{}.toml", dir_path, port), toml_str)
    }
}
