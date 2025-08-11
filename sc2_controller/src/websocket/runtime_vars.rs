use indexmap::IndexSet;
use sc2_proto::sc2api::Request;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::game::game_config::GameConfig;
use crate::game::player_result::PlayerResult;
use crate::game::sc2_result::Sc2Result;

pub struct RuntimeVars {
    pub timeout_secs: Duration,
    pub replay_path: PathBuf,
    pub start_timer: bool,
    pub start_time: Instant,
    pub avg_frame_time: f32,
    pub surrender: bool,
    pub tags: IndexSet<String>,
    pub player_id: Option<u32>,
    pub game_loops: u32,
    pub frame_time: f32,
}

impl RuntimeVars {
    #[must_use]
    pub fn new(config: &GameConfig) -> Self {
        let replay_dir = std::path::Path::new(config.replay_path());
        let replay_path = replay_dir.join(&config.replay_name);
        Self {
            timeout_secs: Duration::from_secs(config.timeout_secs),
            replay_path,
            start_timer: false,
            start_time: Instant::now(),
            avg_frame_time: 0_f32,
            surrender: false,
            tags: IndexSet::with_capacity(10),
            player_id: None,
            game_loops: 0,
            frame_time: 0.0,
        }
    }

    pub fn start_timing(&mut self) {
        self.start_timer = true;
    }
    pub fn start_time(&mut self) {
        self.start_time = Instant::now();
    }
    pub fn record_avg_frame_time(&mut self) {
        self.avg_frame_time = nan_check(self.frame_time / self.game_loops as f32);
    }
    pub fn record_frame_time(&mut self) {
        if self.start_timer {
            self.frame_time += self.start_time.elapsed().as_secs_f32();
        }
    }
    pub fn set_game_loops(&mut self, game_loops: u32) {
        self.game_loops = game_loops;
    }
    pub fn replay_path(&self) -> &str {
        self.replay_path.to_str().unwrap()
    }
    pub fn set_surrender_flag(&mut self) {
        self.surrender = true;
    }
    pub fn player_id(&self) -> u32 {
        self.player_id.unwrap()
    }

    pub fn set_player_id(&mut self, player_id: u32) {
        self.player_id = Some(player_id);
    }

    pub fn add_tags(&mut self, request: &Request) {
        for tag in request
            .action()
            .actions
            .iter()
            .filter(|a| a.action_chat.has_message())
            .filter_map(|x| {
                let msg = x.action_chat.message();
                if msg.contains("Tag:") {
                    msg.strip_prefix("Tag:").map(String::from)
                } else {
                    None
                }
            })
        {
            self.tags.insert(tag);
        }
    }
    pub fn build_result(self, result: Sc2Result) -> PlayerResult {
        PlayerResult {
            game_loops: self.game_loops,
            frame_time: self.avg_frame_time,
            player_id: self.player_id.unwrap(),
            tags: self.tags,
            result,
        }
    }
}

fn nan_check(number: f32) -> f32 {
    if number.is_nan() {
        0f32
    } else {
        number
    }
}
