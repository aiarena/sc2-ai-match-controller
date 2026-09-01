use crate::request::MatchRequest;
use crate::result::MatchResult;
use crate::settings::Settings;
use crate::signals::{bot1_signal_path, bot2_signal_path, delete_signal};

pub async fn delete_all_signals(settings: &Settings) {
    MatchRequest::delete_file().expect("Failed to delete previous match request");
    MatchResult::delete_file().expect("Failed to delete previous match result");
    delete_signal(&bot1_signal_path(settings)).await;
    delete_signal(&bot2_signal_path(settings)).await;
}
