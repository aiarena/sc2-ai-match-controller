use crate::matches::aiarena_result::AiArenaResult;
use crate::matches::sources::file_source::errors::{FileMatchExtractError, SubmissionError};
use crate::matches::sources::file_source::open_results_file;
use crate::matches::sources::{AiArenaGameResult, LogsAndReplays, MatchSource};
use crate::matches::{Match, MatchPlayer};
use common::async_trait::async_trait;
use common::configuration::ac_config::ACConfig;
use common::models::bot_controller::PlayerNum;
use common::parking_lot::RwLock;
use common::tracing::log::error;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Lines, Read, Seek, SeekFrom, Write};
use std::str::FromStr;

pub struct TestSource {
    settings: ACConfig,
    expected_result: RwLock<Option<AiArenaResult>>,
    temp_matches_file: String,
}

impl TestSource {
    pub fn new(settings: ACConfig) -> Self {
        Self {
            settings,
            expected_result: RwLock::new(None),
            temp_matches_file: "test-matches".to_string(),
        }
    }
    fn update_matches_file(&self) -> Result<(), SubmissionError> {
        if let Ok(lines) = self.read_matches_file() {
            let mut line_vec: Vec<String> = lines.map(Result::unwrap).collect();
            for line in &mut line_vec {
                if !line.is_empty() && !line.starts_with('#') {
                    line.insert(0, '#');
                    break;
                }
            }
            let file =
                File::create(&self.temp_matches_file).map_err(SubmissionError::FileCreate)?;
            let mut writer = BufWriter::new(file);
            for line in line_vec {
                writeln!(writer, "{}", line).map_err(SubmissionError::FileWrite)?;
            }
        }
        Ok(())
    }
    fn update_results_file(
        game_result: &AiArenaGameResult,
        results_file_path: &str,
    ) -> Result<(), SubmissionError> {
        let mut results_file = open_results_file(results_file_path)?;
        let mut bytes = Vec::with_capacity(1000);
        results_file
            .read_to_end(&mut bytes)
            .map_err(SubmissionError::FileRead)?;

        let mut results = match serde_json::from_slice::<Results>(&bytes) {
            Ok(r) => r,
            Err(e) => {
                println!("{:?}", e);
                Results::default()
            }
        };
        results.results.push(game_result.clone());
        results_file.set_len(0).map_err(SubmissionError::Truncate)?;
        results_file
            .seek(SeekFrom::Start(0))
            .map_err(SubmissionError::Seek)?;
        results_file
            .write_all(
                &serde_json::to_vec_pretty(&results).map_err(SubmissionError::Serialization)?,
            )
            .map_err(SubmissionError::FileWrite)?;
        Ok(())
    }
    fn get_current_match_id(results_file_path: &str) -> u32 {
        if let Ok(mut results_file) = open_results_file(results_file_path) {
            let mut bytes = Vec::with_capacity(1000);
            results_file
                .read_to_end(&mut bytes)
                .map_err(SubmissionError::FileRead)
                .unwrap_or(0);

            let results = match serde_json::from_slice::<Results>(&bytes) {
                Ok(r) => r,
                Err(_) => Results::default(),
            };
            return results
                .results
                .iter()
                .max_by(|x, y| x.match_id.cmp(&y.match_id))
                .map_or(0, |x| x.match_id);
        }

        0u32
    }
    fn read_matches_file(&self) -> std::io::Result<Lines<BufReader<File>>> {
        let matches_path = std::path::Path::new(&self.settings.matches_file);
        let new_matches_file = matches_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("/"))
            .join(&self.temp_matches_file);
        if !new_matches_file.exists() {
            std::fs::copy(matches_path, &new_matches_file)?;
        }
        let file = File::open(&new_matches_file)?;
        let reader = BufReader::new(file);
        Ok(reader.lines())
    }
}
#[async_trait]
impl MatchSource for TestSource {
    async fn has_next(&self) -> bool {
        if let Ok(lines) = self.read_matches_file() {
            for line in lines.flatten() {
                if !line.is_empty() && !line.starts_with('#') {
                    return true;
                }
            }
        }

        false
    }

    async fn next_match(&self) -> Option<Match> {
        if let Ok(lines) = self.read_matches_file() {
            for line in lines.flatten() {
                if !line.is_empty() && !line.starts_with('#') {
                    return match extract_match(&line) {
                        Ok((mut m, expected_result)) => {
                            m.match_id =
                                Self::get_current_match_id(&self.settings.results_file) + 1;
                            *self.expected_result.write() = Some(expected_result);
                            Some(m)
                        }
                        Err(e) => {
                            error!("{:?}", e);
                            None
                        }
                    };
                }
            }
        }

        None
    }

    async fn submit_result(
        &self,
        game_result: &AiArenaGameResult,
        _logs_and_replays: Option<LogsAndReplays>,
    ) -> Result<(), SubmissionError> {
        //TODO: logs
        let expected_result = self.expected_result.read().unwrap();
        if expected_result != game_result.result {
            error!(
                "Actual result {:?} does not match expected result {:?}",
                expected_result, game_result.result
            );
            std::process::exit(2);
        }
        Self::update_results_file(game_result, &self.settings.results_file)?;
        self.update_matches_file()?;

        Ok(())
    }
}

#[derive(Deserialize, Serialize, Default, Debug)]
struct Results {
    results: Vec<AiArenaGameResult>,
}

fn extract_match(line: &str) -> Result<(Match, AiArenaResult), FileMatchExtractError> {
    let mut vec_line: Vec<String> = line
        .split(',')
        .map(std::string::ToString::to_string)
        .collect();

    match vec_line.len().cmp(&10) {
        Ordering::Greater => {
            return Err(FileMatchExtractError::TooManyFields(vec_line));
        }
        Ordering::Less => {
            return Err(FileMatchExtractError::MissingFields(vec_line));
        }
        Ordering::Equal => {}
    }

    let bot1: Vec<String> = vec_line.drain(0..4).collect();
    let bot2: Vec<String> = vec_line.drain(0..4).collect();

    let expected_result = vec_line
        .pop()
        .map(|r| AiArenaResult::from_str(&r).unwrap())
        .ok_or_else(|| FileMatchExtractError::MapName(vec_line.clone()))?;

    let map_name = vec_line
        .pop()
        .ok_or_else(|| FileMatchExtractError::MapName(vec_line.clone()))?;
    let players = HashMap::from([
        (PlayerNum::One, MatchPlayer::from_file_source(&bot1)?),
        (PlayerNum::Two, MatchPlayer::from_file_source(&bot2)?),
    ]);
    Ok((
        Match {
            match_id: 0,
            players,
            map_name,
            aiarena_match: None,
        },
        expected_result,
    ))
}

#[cfg(test)]
mod tests {
    use crate::game::race::BotRace;
    use crate::matches::aiarena_result::AiArenaResult;
    use crate::matches::sources::file_source::errors::FileMatchExtractError;
    use crate::matches::sources::test_source::extract_match;
    use common::models::bot_controller::PlayerNum;

    #[test]
    pub fn test_match_extracts_valid() {
        let m = extract_match(
            "bot-id-1,basic_bot,T,python,bot-id-2,loser_bot,P,python,AutomatonLE,Player1Win",
        );
        assert!(m.is_ok());
        let (m, expected_result) = m.unwrap();
        assert_eq!(m.players[&PlayerNum::One].id, "bot-id-1");
        assert_eq!(m.players[&PlayerNum::Two].id, "bot-id-2");
        assert_eq!(m.players[&PlayerNum::One].name, "basic_bot");
        assert_eq!(m.players[&PlayerNum::Two].name, "loser_bot");
        assert_eq!(m.players[&PlayerNum::One].race, BotRace::Terran);
        assert_eq!(m.players[&PlayerNum::Two].race, BotRace::Protoss);
        assert_eq!(m.map_name, "AutomatonLE");
        assert_eq!(expected_result, AiArenaResult::Player1Win);
    }

    #[test]
    pub fn test_match_extracts_invalid_extra_field() {
        let m = extract_match(
            "AutomatonLE,AutomatonLE,basic_bot,T,python,bot-id-2,loser_bot,P,python,AutomatonLE,AutomatonLE",
        );
        assert!(m.is_err());
        let m_err = m.err().unwrap();
        assert!(matches!(m_err, FileMatchExtractError::TooManyFields { .. }));
    }
}
