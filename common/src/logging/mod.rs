use tracing_appender::non_blocking::NonBlocking;
use tracing_appender::rolling::RollingFileAppender;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init_logging(
    rust_log: &str,
    non_blocking_stdout: NonBlocking,
    non_blocking_file: RollingFileAppender,
) {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(rust_log))
        //file
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_file)
                .with_file(true)
                .with_ansi(false)
                .with_line_number(true)
                .with_target(false),
        )
        //stdout
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_stdout)
                .with_file(true)
                .with_line_number(true)
                .with_target(false),
        )
        .init();
}
