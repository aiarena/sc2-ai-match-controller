mod graphql;
mod logging;
mod prepare;
mod race;
mod request;
mod result;
mod settings;
mod signals;
mod submit;

use crate::settings::RunType;

#[tokio::main]
async fn main() {
    let settings = settings::load();
    let _logging = logging::init(&settings).await;

    match settings.run_type {
        RunType::Prepare => prepare::prepare_match(&settings).await,
        RunType::Submit => submit::submit_result(&settings).await,
    }

    if settings.keep_alive {
        println!("Keep-alive mode enabled. Waiting for termination signal...");

        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
            tokio::select! {
                _ = sigterm.recv() => {
                    println!("Received SIGTERM, shutting down gracefully");
                }
            }
        }
    }

    println!("Match controller exits");
}
