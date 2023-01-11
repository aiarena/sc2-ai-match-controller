use crate::matches::Match;
use common::async_trait::async_trait;
use common::tracing::debug;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod aiarena_api;
pub mod file_source;
pub mod test_source;

use crate::game::game_result::GameResult;
use crate::game::sc2_result::Sc2Result;
use crate::matches::aiarena_result::AiArenaResult;
use crate::matches::sources::file_source::errors::SubmissionError;
pub use file_source::FileSource;

#[async_trait]
pub trait MatchSource: Sync + Send {
    async fn has_next(&self) -> bool;
    async fn next_match(&self) -> Option<Match>;
    async fn submit_result(
        &self,
        game_result: &AiArenaGameResult,
        logs_and_replays: Option<LogsAndReplays>,
    ) -> Result<(), SubmissionError>;
}

#[async_trait]
impl<T: MatchSource + ?Sized> MatchSource for Box<T> {
    async fn has_next(&self) -> bool {
        (**self).has_next().await
    }

    async fn next_match(&self) -> Option<Match> {
        (**self).next_match().await
    }

    async fn submit_result(
        &self,
        game_result: &AiArenaGameResult,
        logs_and_replays: Option<LogsAndReplays>,
    ) -> Result<(), SubmissionError> {
        (**self).submit_result(game_result, logs_and_replays).await
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiArenaGameResult {
    #[serde(rename = "match")]
    match_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    bot1_avg_step_time: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bot1_tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bot2_avg_step_time: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bot2_tags: Option<Vec<String>>,
    #[serde(rename = "type")]
    result: AiArenaResult,
    game_steps: u32,
}

impl From<&GameResult> for AiArenaGameResult {
    fn from(game_result: &GameResult) -> Self {
        let mut game_steps = 0;
        let mut bot1_avg_step_time = None;
        let mut bot1_tags = None;
        let mut bot2_avg_step_time = None;
        let mut bot2_tags = None;
        let mut p1_result = None;
        let mut p2_result = None;

        match &game_result.player1_result {
            None => {}
            Some(player1_result) => {
                debug!("Player1Result: {:?}", player1_result);
                bot1_avg_step_time = Some(player1_result.frame_time);
                bot1_tags = Some(player1_result.tags.iter().cloned().collect());
                game_steps = player1_result.game_loops;
                p1_result = Some(player1_result.result);
            }
        }
        match &game_result.player2_result {
            None => {}
            Some(player2_result) => {
                debug!("Player2Result: {:?}", player2_result);
                bot2_avg_step_time = Some(player2_result.frame_time);
                bot2_tags = Some(player2_result.tags.iter().cloned().collect());
                game_steps = player2_result.game_loops;
                p2_result = Some(player2_result.result);
            }
        }
        let result = game_result
            .result
            .unwrap_or_else(|| match (p1_result, p2_result) {
                (Some(Sc2Result::SC2Crash), _) | (_, Some(Sc2Result::SC2Crash)) => {
                    AiArenaResult::Error
                }
                (Some(Sc2Result::Tie), _) | (_, Some(Sc2Result::Tie)) => AiArenaResult::Tie,
                (Some(Sc2Result::Crash), _) => AiArenaResult::Player1Crash,
                (_, Some(Sc2Result::Crash)) => AiArenaResult::Player2Crash,
                (Some(Sc2Result::Timeout), _) => AiArenaResult::Player1TimeOut,
                (_, Some(Sc2Result::Timeout)) => AiArenaResult::Player2TimeOut,
                (Some(Sc2Result::Victory), _) | (_, Some(Sc2Result::Defeat)) => {
                    AiArenaResult::Player1Win
                }
                (_, Some(Sc2Result::Victory)) | (Some(Sc2Result::Defeat), _) => {
                    AiArenaResult::Player2Win
                }
                #[cfg(test)]
                (Some(Sc2Result::Placeholder), Some(Sc2Result::Placeholder)) => unreachable!(),
                (_, _) => unreachable!(),
            });
        Self {
            match_id: game_result.match_id,
            bot1_avg_step_time,
            bot1_tags,
            bot2_avg_step_time,
            bot2_tags,
            result,
            game_steps,
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogsAndReplays {
    pub bot1_dir: PathBuf,
    pub bot2_dir: PathBuf,
    pub arenaclient_log: PathBuf,
    pub replay_file: PathBuf,
}

#[cfg(test)]
mod tests {
    use crate::game::game_result::GameResult;
    use crate::game::player_result::PlayerResult;
    use crate::game::sc2_result::Sc2Result;
    use crate::matches::aiarena_result::AiArenaResult;
    use crate::matches::sources::AiArenaGameResult;

    fn game_result() -> GameResult {
        GameResult {
            match_id: 0,
            player1_result: Some(PlayerResult {
                game_loops: 0,
                frame_time: 0.0,
                player_id: 0,
                tags: Default::default(),
                result: Sc2Result::Placeholder,
            }),
            player2_result: Some(PlayerResult {
                game_loops: 0,
                frame_time: 0.0,
                player_id: 0,
                tags: Default::default(),
                result: Sc2Result::Placeholder,
            }),
            result: Some(AiArenaResult::Placeholder),
        }
    }

    #[test]
    fn test_result_serialization_error() {
        let mut game_result = game_result();
        game_result.result = Some(AiArenaResult::Error);
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Error");
    }

    #[test]
    fn test_result_serialization_p1_victory() {
        let mut game_result = game_result();
        game_result.player1_result.as_mut().unwrap().result = Sc2Result::Victory;
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Defeat;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Player1Win");
    }

    #[test]
    fn test_result_serialization_p2_victory() {
        let mut game_result = game_result();
        game_result.player1_result.as_mut().unwrap().result = Sc2Result::Defeat;
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Victory;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Player2Win");
    }

    #[test]
    fn test_result_serialization_p1_timeout() {
        let mut game_result = game_result();
        game_result.player1_result.as_mut().unwrap().result = Sc2Result::Timeout;
        game_result.player2_result = None;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Player1TimeOut");
    }

    #[test]
    fn test_result_serialization_p2_timeout() {
        let mut game_result = game_result();
        game_result.player1_result = None;
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Timeout;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Player2TimeOut");
    }

    #[test]
    fn test_result_serialization_tie() {
        let mut game_result = game_result();
        game_result.player1_result.as_mut().unwrap().result = Sc2Result::Tie;
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Tie;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Tie");
    }

    #[test]
    fn test_result_serialization_sc2_crash() {
        let mut game_result = game_result();
        game_result.player1_result.as_mut().unwrap().result = Sc2Result::SC2Crash;
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Victory;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Error");
    }

    #[test]
    fn test_result_serialization_p1_crash() {
        let mut game_result = game_result();
        game_result.player1_result.as_mut().unwrap().result = Sc2Result::Crash;
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Victory;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Player1Crash");
    }

    #[test]
    fn test_result_serialization_p2_crash() {
        let mut game_result = game_result();
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Victory;
        game_result.player2_result.as_mut().unwrap().result = Sc2Result::Crash;
        game_result.result = None;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["type"], "Player2Crash");
    }

    #[test]
    fn test_result_serialization_match_id() {
        let mut game_result = game_result();
        let match_id = 10;
        game_result.match_id = match_id;
        let aiarena_game_result = AiArenaGameResult::from(&game_result);
        let serialized =
            serde_json::to_value(&aiarena_game_result).expect("Could not serialize GameResult");
        assert_eq!(serialized["match"], match_id);
    }

    // #[test]
    // fn test_game_result_serialization() {
    //     let match_id = 9999;
    //     let game_time = 8888;
    //     let p1_result = Sc2Result::Victory;
    //     let p2_result = Sc2Result::Defeat;
    //     let p1_frame_time = 22f32;
    //     let p2_frame_time = 23f32;
    //     let mut p1_tags = IndexSet::new();
    //     let mut p2_tags = p1_tags.clone();
    //     p1_tags.insert("123".to_string());
    //     p2_tags.insert("456".to_string());
    //     p2_tags.insert("789".to_string());
    //
    //
    //     let s = serde_json::to_string(&game_result)
    //         .expect("Could not serialize GameResult");
    //     println!("{:?}", s);
    // }
}
