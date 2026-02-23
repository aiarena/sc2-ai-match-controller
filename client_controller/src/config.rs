use config::{Config, Environment, File, FileFormat};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ControllerConfig {
    pub version: String,
    pub api_url: String,
    pub game_controller: String,
    pub bot_controller: String,
    pub gamesets_directory: String,
    pub bots_directory: String,
    pub logs_directory: String,
    pub matches_file: String,
}

pub fn initialize_config() -> ControllerConfig {
    let mut config = Config::builder()
        .add_source(File::from_str(include_str!("config.toml"), FileFormat::Toml).required(true))
        .add_source(File::new("config.toml", FileFormat::Toml).required(false))
        .add_source(File::new("config.json", FileFormat::Json).required(false))
        .add_source(Environment::default())
        .build()
        .expect("Could not load the client controller configuration")
        .try_deserialize::<ControllerConfig>()
        .expect("Could not deserialize the client controller configuration");

    // Convert bots_directory to absolute path
    let bots_path = Path::new(&config.bots_directory);
    if bots_path.is_relative() {
        config.bots_directory = std::env::current_dir()
            .expect("Failed to get current directory")
            .join(bots_path)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/");
    } else {
        config.bots_directory = config.bots_directory.replace('\\', "/");
    }

    config
}

#[derive(Debug, Clone, Default)]
pub struct Bot {
    pub id: String,
    pub name: String,
    pub runtype: String,
    pub base: String,
}

#[derive(Debug, Clone, Default)]
pub struct MatchRequest {
    pub bot1: Bot,
    pub bot2: Bot,
}

impl MatchRequest {

    pub fn from_csv_line(line: &str) -> Self {
        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        let (bot1_type, bot1_base) = split(parts[3]);
        let (bot2_type, bot2_base) = split(parts[7]);

        Self {
            bot1: Bot {
                id: parts[0].to_string(),
                name: parts[1].to_string(),
                runtype: bot1_type,
                base: bot1_base,
            },
            bot2: Bot {
                id: parts[4].to_string(),
                name: parts[5].to_string(),
                runtype: bot2_type,
                base: bot2_base,
            },
        }
    }

}

fn split(raw_bot_type: &str) -> (String, String) {
    let (bot_type, bot_base) = if raw_bot_type.contains('@') {
        let mut parts = raw_bot_type.split('@');
        let bot_type = parts.next().unwrap_or("").to_string();
        let bot_base = parts.next().unwrap_or("").to_string();
        (bot_type, bot_base)
    } else {
        (raw_bot_type.to_string(), String::new())
    };

    (bot_type, bot_base)
}
