use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

use crate::settings::Settings;
use crate::signals::{bot1_signal_path, bot2_signal_path};

pub async fn check_bots_started(settings: &Settings) -> bool {
    // Give time for bots to start
    sleep(Duration::from_secs(10)).await;

    // Return false if either exited prematurely
    !bot1_signal_path(settings).exists() && !bot2_signal_path(settings).exists()
}

pub async fn wait_for_bots_to_terminate(settings: &Settings) {
    let bot1_signal = bot1_signal_path(settings);
    let bot2_signal = bot2_signal_path(settings);
    let start_time = std::time::Instant::now();
    loop {
        let bot1_exited = bot1_signal.exists();
        let bot2_exited = bot2_signal.exists();
        if bot1_exited && bot2_exited {
            return;
        }
        if start_time.elapsed() >= Duration::from_secs(60) {
            if !bot1_exited {
                info!("Bot 1 did not terminate within 60 seconds");
            }
            if !bot2_exited {
                info!("Bot 2 did not terminate within 60 seconds");
            }
            return;
        }
        sleep(Duration::from_secs(1)).await;
    }
}
