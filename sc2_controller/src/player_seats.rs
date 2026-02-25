use crate::game::game_result::GameResult;
use crate::websocket::player::Player;
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::oneshot::Sender;

pub struct PlayerSeat {
    pub player_num: u8,
    pub pass_port: u32,

    // The port exposed to players
    pub external_port: u16,

    // The port to the SC2 process
    // TODO: Use fixed internal port instead
    pub internal_port: u16,

    pub game_result: Arc<RwLock<GameResult>>,

    pub channel: Player,

    pub completion_tx: Arc<Mutex<Option<Sender<()>>>>,
}

impl PlayerSeat {
    pub fn new(num: u8, game_result: Arc<RwLock<GameResult>>, completion_tx: Sender<()>) -> Self {
        PlayerSeat {
            player_num: num,
            pass_port: get_pass_port(num),
            external_port: get_external_port(num),
            internal_port: 0,
            game_result,
            channel: Player::new(),
            completion_tx: Arc::new(Mutex::new(Some(completion_tx))),
        }
    }
}

fn get_external_port(num: u8) -> u16 {
    let env_var = format!("PLAYER_{}_SEAT", num);
    let value = std::env::var(&env_var).unwrap_or_else(|_| {
        panic!("Missing {} environment variable", env_var);
    });
    value.parse().unwrap_or_else(|_| {
        panic!("Invalid {} environment variable", env_var);
    })
}

fn get_pass_port(num: u8) -> u32 {
    let env_var = format!("PLAYER_{}_PASS", num);
    let value = std::env::var(&env_var).unwrap_or_else(|_| get_external_port(num).to_string());
    value.parse().unwrap_or_else(|_| {
        panic!("Invalid {} environment variable", env_var);
    })
}
