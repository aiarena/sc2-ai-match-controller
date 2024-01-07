use reqwest::multipart::Form;
use std::path::Path;
use tracing::error;

use crate::models::aiarena::aiarena_game_result::AiArenaGameResult;
use crate::PlayerNum;

pub mod aiarena_api_client;
pub mod errors;

pub struct AiArenaResultForm {
    inner: Form,
}

impl AiArenaResultForm {
    pub fn to_inner(self) -> Form {
        self.inner
    }
    pub async fn add_bot_data(self, player_num: PlayerNum, bot_data_path: &Path) -> Self {
        match create_part_from_path(bot_data_path).await {
            Ok(part) => {
                let part_name = match player_num {
                    PlayerNum::One => "bot1_data",
                    PlayerNum::Two => "bot2_data",
                };
                Self {
                    inner: self.inner.part(part_name, part),
                }
            }
            Err(e) => {
                error!("{:?}", e);
                self
            }
        }
    }
    pub async fn add_bot_log(self, player_num: PlayerNum, bot_log_path: &Path) -> Self {
        match create_part_from_path(bot_log_path).await {
            Ok(part) => {
                let part_name = match player_num {
                    PlayerNum::One => "bot1_log",
                    PlayerNum::Two => "bot2_log",
                };
                Self {
                    inner: self.inner.part(part_name, part),
                }
            }
            Err(e) => {
                error!("{:?}", e);
                self
            }
        }
    }
    pub async fn add_replay(self, replay_path: &Path) -> Self {
        match create_part_from_path(replay_path).await {
            Ok(part) => Self {
                inner: self.inner.part("replay_file", part),
            },
            Err(e) => {
                error!("{:?}", e);
                self
            }
        }
    }
    pub async fn add_arenaclient_logs(self, arenaclient_log_path: &Path) -> Self {
        match create_part_from_path(arenaclient_log_path).await {
            Ok(part) => Self {
                inner: self.inner.part("arenaclient_log", part),
            },
            Err(e) => {
                error!("{:?}", e);
                self
            }
        }
    }
    fn add_avg_step_time(self, player_num: PlayerNum, avg_step_time: Option<f32>) -> Self {
        if let Some(avg_step_time) = avg_step_time {
            let avg_step_time = if avg_step_time.is_finite() {
                avg_step_time
            } else {
                0f32
            };
            let part_name = match player_num {
                PlayerNum::One => "bot1_avg_step_time",
                PlayerNum::Two => "bot2_avg_step_time",
            };
            Self {
                inner: self.inner.text(part_name, avg_step_time.to_string()),
            }
        } else {
            self
        }
    }
    fn add_avg_step_times(self, game_result: &AiArenaGameResult) -> Self {
        self.add_avg_step_time(PlayerNum::One, game_result.bot1_avg_step_time)
            .add_avg_step_time(PlayerNum::Two, game_result.bot2_avg_step_time)
    }
    fn add_bot_tag(mut self, player_num: PlayerNum, bot_tags: Option<&Vec<String>>) -> Self {
        if let Some(bot1_tags) = bot_tags {
            let part_name = match player_num {
                PlayerNum::One => "bot1_tags",
                PlayerNum::Two => "bot2_tags",
            };
            for tag in bot1_tags {
                self.inner = self
                    .inner
                    .text(part_name, serde_json::to_string(&tag).unwrap());
            }
            Self { inner: self.inner }
        } else {
            self
        }
    }

    fn add_bot_tags(self, game_result: &AiArenaGameResult) -> Self {
        self.add_bot_tag(PlayerNum::One, game_result.bot1_tags.as_ref())
            .add_bot_tag(PlayerNum::Two, game_result.bot2_tags.as_ref())
    }
}

impl From<&AiArenaGameResult> for AiArenaResultForm {
    fn from(game_result: &AiArenaGameResult) -> Self {
        let form = Form::new()
            .text("match", game_result.match_id.to_string())
            .text("type", game_result.result.to_string())
            .text("game_steps", game_result.game_steps.to_string());

        let ret_value = Self { inner: form };
        ret_value
            .add_avg_step_times(game_result)
            .add_bot_tags(game_result)
    }
}

pub async fn create_part_from_path(
    path: &Path,
) -> Result<reqwest::multipart::Part, std::io::Error> {
    let file_name = String::from(path.file_name().and_then(|p| p.to_str()).unwrap());
    let file = tokio::fs::read(path).await?;

    let file_part = reqwest::multipart::Part::bytes(file).file_name(file_name);
    Ok(file_part)
}

pub fn create_part_from_bytes(bytes: Vec<u8>, filename: String) -> reqwest::multipart::Part {
    let file_part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
    file_part
}
