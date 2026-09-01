mod arena_api;
mod arenaclient;
mod k8s_config;
mod k8s_processor;
mod profile;
mod templating;

use crate::k8s_processor::process;
use config::{Config, FileFormat};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use std::path::Path;

static VERSION: &str = env!("CARGO_PKG_VERSION");
static PREFIX: &str = "ACK8S";

#[tokio::main]
async fn main() {
    rustls::crypto::ring::default_provider().install_default().expect("Failed to install rustls crypto provider");

    let env_log = std::env::var("RUST_LOG").unwrap_or_else(|_| format!("info,common={},k8s_controller={}", "debug", "debug"));
    let log_path = "/logs/k8s_controller".to_string();
    let log_file = "k8s_controller.log";
    let full_path = Path::new(&log_path).join(log_file);
    if full_path.exists() {
        tokio::fs::remove_file(full_path).await.unwrap();
    }
    let settings = setup_k8s_config();

    let (non_blocking_stdout, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let non_blocking_file = tracing_appender::rolling::never(&log_path, log_file);
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&env_log))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_file)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_target(false),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_stdout)
                .with_file(true)
                .with_line_number(true)
                .with_target(false),
        )
        .init();

    info!("Running version: {:?}", VERSION);

    process(settings).await;
}

fn setup_k8s_config() -> k8s_config::K8sConfig {
    let default_config = include_str!("../configs/default_config.toml");
    Config::builder()
        .add_source(config::File::from_str(default_config, FileFormat::Toml).required(true))
        .add_source(config::File::new("config.toml", FileFormat::Toml).required(false))
        .add_source(config::File::new("config.json", FileFormat::Json).required(false))
        .add_source(config::Environment::default().prefix(PREFIX))
        .build()
        .expect("Could not load config")
        .try_deserialize::<k8s_config::K8sConfig>()
        .expect("Could not deserialize config")
}
