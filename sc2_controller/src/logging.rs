use std::fs::{File, OpenOptions};
use std::path::Path;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

fn create_log_file(file_name: &str) -> File {
    let log_folder = std::env::var("LOG_FOLDER").unwrap_or_else(|_| "/logs".into());
    let log_path = Path::new(&log_folder).join(file_name);

    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
        .expect(&format!("Could not create file {}", file_name))
}

pub fn init_logs() -> (
    tracing_appender::non_blocking::WorkerGuard,
    tracing_appender::non_blocking::WorkerGuard,
) {
    let controller_logs = create_log_file("sc2_controller.log");

    let (non_blocking_stdout, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());
    let (non_blocking_controller_logs, controller_logs_guard) =
        tracing_appender::non_blocking(controller_logs);

    let env_filter =
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info,sc2_controller=info".into());

    tracing_subscriber::registry()
        .with(EnvFilter::new(env_filter))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_controller_logs)
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

    info!("Controller logs initialized.");
    (stdout_guard, controller_logs_guard)
}
