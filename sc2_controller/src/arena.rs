use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlayerNum {
    One,
    Two,
}

impl PlayerNum {
    pub fn other_player(&self) -> PlayerNum {
        match self {
            PlayerNum::One => PlayerNum::Two,
            PlayerNum::Two => PlayerNum::One,
        }
    }
}

#[derive(PartialOrd, PartialEq, Eq, Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BotRace {
    NoRace = 0,
    Terran = 1,
    Zerg = 2,
    Protoss = 3,
    Random = 4,
}

impl BotRace {
    pub fn from_str(race: &str) -> Self {
        match &race.to_lowercase()[..] {
            "p" | "protoss" | "race.protoss" | "3" => Self::Protoss,
            "t" | "terran" | "race.terran" | "1" => Self::Terran,
            "r" | "random" | "race.random" | "4" => Self::Random,
            "z" | "zerg" | "race.zerg" | "2" => Self::Zerg,
            _ => Self::NoRace,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchPlayer {
    pub id: String,
    pub name: String,
    pub race: BotRace,
    pub bot_type: String,
    pub bot_base: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MatchRequest {
    pub match_id: u32,
    pub player_1_id: String,
    pub player_1_name: String,
    pub player_2_id: String,
    pub player_2_name: String,
    pub map_name: String,
    pub player_1_race: u8,
    pub player_2_race: u8,
}

impl MatchRequest {
    pub fn read() -> Self {
        config::Config::builder()
            .add_source(
                config::File::new("/match/match-request.toml", config::FileFormat::Toml)
                    .required(false),
            )
            .add_source(config::Environment::default())
            .build()
            .expect("Could not read match request data")
            .try_deserialize::<MatchRequest>()
            .expect("Could not parse match request data")
    }

    pub fn write(&self) -> Result<(), std::io::Error> {
        let toml_str = toml::to_string(self).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "Could not serialize match request data",
            )
        })?;
        tracing::debug!("Writing match request to file: {}", toml_str);

        std::fs::create_dir_all("/match")?;
        std::fs::write("/match/match-request.toml", toml_str)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Eq, PartialEq)]
pub enum AiArenaResult {
    Player1Crash,
    Player2Crash,
    Player1TimeOut,
    Player2TimeOut,
    Player1Win,
    Player2Win,
    Tie,
    InitializationError,
    Error,
    Placeholder,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiArenaGameResult {
    #[serde(rename = "match")]
    pub match_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot1_avg_step_time: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot1_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot2_avg_step_time: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bot2_tags: Option<Vec<String>>,
    #[serde(rename = "type")]
    pub result: AiArenaResult,
    pub game_steps: u32,
}

impl AiArenaGameResult {
    pub fn new_initialization_error(match_id: u32) -> Self {
        AiArenaGameResult {
            match_id,
            bot1_avg_step_time: None,
            bot1_tags: None,
            bot2_avg_step_time: None,
            bot2_tags: None,
            result: AiArenaResult::InitializationError,
            game_steps: 0,
        }
    }

    pub fn from_json_file() -> Result<Self, Box<dyn Error>> {
        let file = File::open("/match/match_result.json")?;
        let reader = BufReader::new(file);
        let result = serde_json::from_reader(reader)?;
        Ok(result)
    }

    pub fn to_json_file(&self) -> Result<(), Box<dyn Error>> {
        let path = Path::new("/match/match_result.json");

        if path.exists() {
            let record = Self::from_json_file()?;
            println!("Match result already stored: {:?}", record);
            println!("Ignoring new match result: {:?}", self);
            return Ok(());
        }

        let file = File::create(path)?;
        serde_json::to_writer_pretty(file, &self)?;
        Ok(())
    }

    pub fn delete_json_file() -> Result<(), Box<dyn Error>> {
        let path = Path::new("/match/match_result.json");
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
