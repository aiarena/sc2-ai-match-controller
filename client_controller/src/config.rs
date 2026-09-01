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

    // Convert to absolute paths with forward slashes so Docker volume mounts work correctly.
    // logs_directory is intentionally left as-is: it is used as a relative volume path in
    // docker-compose.yaml (located in target/), so Docker resolves it relative to that file.
    config.bots_directory = normalize_path(&config.bots_directory);
    config.gamesets_directory = normalize_path(&config.gamesets_directory);

    config
}

fn normalize_path(p: &str) -> String {
    let path = Path::new(p);
    if path.is_relative() {
        std::env::current_dir()
            .expect("Failed to get current directory")
            .join(path)
            .to_string_lossy()
            .to_string()
            .replace('\\', "/")
    } else {
        p.replace('\\', "/")
    }
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
    pub expected_result: Option<String>,
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
            expected_result: parts.get(9).filter(|s| !s.is_empty()).map(|s| s.to_string()),
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
