use std::path::PathBuf;

use crate::settings::Settings;

pub fn bot1_signal_path(settings: &Settings) -> PathBuf {
    PathBuf::from(&settings.log_root).join("bot-controller-1").join("signal.exit")
}

pub fn bot2_signal_path(settings: &Settings) -> PathBuf {
    PathBuf::from(&settings.log_root).join("bot-controller-2").join("signal.exit")
}

pub async fn delete_signal(path: &PathBuf) {
    match tokio::fs::remove_file(path).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => panic!("Failed to clear signal {:?}: {}", path, e),
    }
}
