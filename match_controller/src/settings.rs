use serde::Deserialize;

const PREFIX: &str = "acmatch";

#[derive(Deserialize, Clone, Debug)]
pub struct Settings {
    pub api_token: String,
    pub base_website_url: String,
    pub bot_directory: String,
    pub caching_server_url: String,
    pub game_directory: String,
    pub keep_alive: bool,
    pub logging_level: String,
    pub log_root: String,
    pub matches_file: String,
    pub run_type: RunType,
}

impl Settings {
    pub fn should_use_arena_api(&self) -> bool {
        !self.base_website_url.is_empty() && !self.api_token.is_empty()
    }

    pub fn should_use_cache(&self) -> bool {
        !self.caching_server_url.is_empty()
    }
}

#[derive(Debug, Copy, Clone, Deserialize, Eq, PartialEq)]
pub enum RunType {
    #[serde(rename = "prepare")]
    Prepare,
    #[serde(rename = "submit")]
    Submit,
}

pub fn load() -> Settings {
    let default_config = include_str!("../config.toml");
    config::Config::builder()
        .add_source(config::File::from_str(default_config, config::FileFormat::Toml).required(true))
        .add_source(config::File::new("config.toml", config::FileFormat::Toml).required(false))
        .add_source(config::Environment::default().prefix(PREFIX))
        .build()
        .expect("Could not load config")
        .try_deserialize::<Settings>()
        .expect("Could not deserialize config")
}
