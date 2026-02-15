#![allow(dead_code)]
mod match_scheduler;
pub mod matches;
mod routes;
mod state;

use crate::match_scheduler::match_scheduler;
use crate::matches::sources::aiarena_api::HttpApiSource;
use crate::matches::sources::test_source::TestSource;
use crate::matches::sources::{FileSource, MatchSource};
use crate::state::ControllerState;
use common::configuration::ac_config::{ACConfig, RunType};
use common::logging::init_logging;
use config::{Config, FileFormat};
use parking_lot::RwLock;
use std::path::Path;
use std::sync::Arc;

static PREFIX: &str = "acmatch";

#[tokio::main]
async fn main() {
    let settings = setup_controller_config();

    let log_level = &settings.logging_level;
    let env_log = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| format!("info,common={log_level},match_controller={log_level}"));
    let log_path = format!("{}/match_controller", &settings.log_root);
    let log_file = "match_controller.log";
    let full_path = Path::new(&log_path).join(log_file);
    if full_path.exists() {
        tokio::fs::remove_file(full_path).await.unwrap();
    }
    let (non_blocking_stdout, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let non_blocking_file = tracing_appender::rolling::never(&log_path, log_file);
    init_logging(&env_log, non_blocking_stdout, non_blocking_file);

    let match_source: Box<dyn MatchSource> = match settings.run_type {
        RunType::Local => Box::new(FileSource::new(settings.clone())),
        RunType::AiArena => Box::new(HttpApiSource::new(settings.clone()).unwrap()),
        RunType::Test => Box::new(TestSource::new(settings.clone())),
    };

    let app_state = Arc::new(RwLock::new(ControllerState {
        settings,
        players: Vec::default(),
        current_match: None,
        map: None,
    }));

    match_scheduler(app_state.clone(), match_source).await;

    println!("Match controller exits");
}

fn setup_controller_config() -> ACConfig {
    let default_config = include_str!("../config.toml");
    Config::builder()
        .add_source(config::File::from_str(default_config, FileFormat::Toml).required(true))
        .add_source(config::File::new("config.toml", FileFormat::Toml).required(false))
        .add_source(config::File::new("config.json", FileFormat::Json).required(false))
        .add_source(config::Environment::default().prefix(PREFIX))
        .build()
        .expect("Could not load config")
        .try_deserialize::<ACConfig>()
        .expect("Could not deserialize config")
}
