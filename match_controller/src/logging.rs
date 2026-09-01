use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::settings::{RunType, Settings};

#[allow(dead_code)]
pub struct LoggingGuard(WorkerGuard);

pub async fn init(settings: &Settings) -> LoggingGuard {
    let log_level = &settings.logging_level;
    let env_log = std::env::var("RUST_LOG").unwrap_or_else(|_| format!("info,match_controller={log_level}"));
    let log_path = format!("{}/match_controller", &settings.log_root);
    let log_file = match settings.run_type {
        RunType::Prepare => "prepare_match.log",
        RunType::Submit => "submit_result.log",
    };
    let full_path = std::path::Path::new(&log_path).join(log_file);
    if full_path.exists() {
        tokio::fs::remove_file(full_path).await.unwrap();
    }
    let (non_blocking_stdout, guard) = tracing_appender::non_blocking(std::io::stdout());
    let file_appender = tracing_appender::rolling::never(&log_path, log_file);

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(&env_log))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
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

    LoggingGuard(guard)
}
