use config::{Environment, File, FileFormat};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub version: String,
    pub gamesets_directory: String,
    pub bots_directory: String,
    pub logs_directory: String,
}

pub fn initialize_config() -> Config {
    config::Config::builder()
        .add_source(File::from_str(include_str!("config.toml"), FileFormat::Toml).required(true))
        .add_source(File::new("config.toml", FileFormat::Toml).required(false))
        .add_source(File::new("config.json", FileFormat::Json).required(false))
        .add_source(Environment::default())
        .build()
        .expect("Could not load the client controller configuration")
        .try_deserialize::<Config>()
        .expect("Could not deserialize the client controller configuration")
}
