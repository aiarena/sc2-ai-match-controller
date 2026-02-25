mod game;
mod logging;
mod player_seats;
mod routes;
mod websocket;
mod ws_routes;

use crate::game::game_config::GameConfig;
use crate::game::game_result::GameResult;
use crate::logging::init_logs;
use crate::player_seats::PlayerSeat;
use crate::routes::{start_sc2_process, start_ws_server};
use crate::ws_routes::connect;
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::MatchRequest;
use common::models::aiarena::aiarena_result::AiArenaResult;
use common::portpicker::pick_unused_port_in_range;
use std::error::Error;
use std::io::{self, ErrorKind};
use std::path::Path;
use std::sync::{Arc, RwLock};
use tokio::sync::{oneshot, Mutex};
use tokio::time::{interval, timeout, Duration, Instant};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let _guards = init_logs();

    let match_request = MatchRequest::read();
    let match_id = match_request.match_id;
    info!("Match request: {:?}", match_request);

    let config = GameConfig::from_file(&match_request);
    let replay_path = Path::new(config.replay_path()).join(&config.replay_name);
    let replay_path_str = replay_path.to_string_lossy();
    info!("Target replay file path: {:?}", replay_path);

    let aiarena_game_result = match run_match(match_request, replay_path_str.as_ref()).await {
        Ok(game_result) => AiArenaGameResult::from(&game_result),
        Err(e) => {
            info!("Match initialization error: {:?}", e);
            AiArenaGameResult::new_initialization_error(match_id)
        }
    };
    info!("Match result: {:?}", &aiarena_game_result);

    match aiarena_game_result.to_json_file() {
        Ok(_) => info!("Match result stored successfully"),
        Err(e) => error!("Match result not stored: {:?}", e),
    }

    info!("Game controller exits");
}

async fn run_match(
    match_request: MatchRequest,
    replay_path: &str,
) -> Result<GameResult, Box<dyn Error>> {
    let match_id = match_request.match_id;
    let match_map = match_request.map_name.clone();
    let game_result = Arc::new(RwLock::new(GameResult::new(match_id)));

    info!("Setting up seat for player 1");
    let (tx1, rx1) = oneshot::channel();
    let mut seat1 = PlayerSeat::new(1, game_result.clone(), tx1);

    // TODO: Use fixed ports instead
    seat1.internal_port = pick_unused_port_in_range(9000..10000)
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "No available port for player 1"))?;

    info!(
        "Starting SC2 process for player 1 listening on port: {:?}",
        seat1.internal_port
    );
    start_sc2_process(&mut seat1).await?;

    let seat1_sc2_ws = connect(seat1.internal_port).await?;
    seat1.channel.connect_game(seat1_sc2_ws);
    seat1.channel.ping_game().await?;

    info!("Creating game on map: {:?}", &match_map);
    seat1.channel.create_game(&match_map, false).await?;

    info!("Opening websocket server for player 1");
    let seat1_external_port = seat1.external_port;
    let seat1_state = Arc::new(Mutex::new(seat1));
    start_ws_server(seat1_external_port, seat1_state.clone()).await?;

    info!("Setting up seat for player 2");
    let (tx2, rx2) = oneshot::channel();
    let mut seat2 = PlayerSeat::new(2, game_result.clone(), tx2);

    // TODO: Use fixed ports instead
    seat2.internal_port = pick_unused_port_in_range(9000..10000)
        .ok_or_else(|| io::Error::new(ErrorKind::NotFound, "No available port for player 2"))?;

    info!(
        "Starting SC2 process for player 2 listening on port: {:?}",
        seat2.internal_port
    );
    start_sc2_process(&mut seat2).await?;

    let seat2_sc2_ws = connect(seat2.internal_port).await?;
    seat2.channel.connect_game(seat2_sc2_ws);
    seat2.channel.ping_game().await?;

    info!("Opening websocket server for player 2");
    let seat2_external_port = seat2.external_port;
    let seat2_state = Arc::new(Mutex::new(seat2));
    start_ws_server(seat2_external_port, seat2_state.clone()).await?;

    info!("Waiting for the players to connect");
    let connect_failure =
        check_players_connected(match_id, seat1_state.clone(), seat2_state.clone()).await?;
    if connect_failure.result.is_some() {
        return Ok(connect_failure);
    }

    info!("Waiting for the match to complete");
    let _ = tokio::join!(rx1, rx2);

    info!("Match is complete");

    if seat1_state
        .lock()
        .await
        .channel
        .save_replay(replay_path)
        .await
    {
        info!("Replay file saved successfully");
    } else {
        info!("No replay file saved");
    }

    let game_result = {
        let guard = game_result
            .read()
            .map_err(|_| io::Error::new(ErrorKind::Other, "Internal error"))?;
        (*guard).clone()
    };

    Ok(game_result)
}

async fn check_players_connected(
    match_id: u32,
    seat1_state: Arc<Mutex<PlayerSeat>>,
    seat2_state: Arc<Mutex<PlayerSeat>>,
) -> Result<GameResult, Box<dyn Error>> {
    let mut game_result = GameResult::new(match_id);
    let timeout_duration = Duration::from_secs(30);
    let mut interval = interval(Duration::from_millis(200));
    let start = Instant::now();

    loop {
        let seat1_connected = match timeout(Duration::from_millis(200), seat1_state.lock()).await {
            Ok(seat1) => seat1.channel.is_player_connected(),
            Err(_) => true, // treat as connected if lock cannot be acquired
        };
        let seat2_connected = match timeout(Duration::from_millis(200), seat2_state.lock()).await {
            Ok(seat2) => seat2.channel.is_player_connected(),
            Err(_) => true, // treat as connected if lock cannot be acquired
        };
        if seat1_connected && seat2_connected {
            info!("Both players connected");
            game_result.result = None;
            return Ok(game_result);
        }
        if start.elapsed() > timeout_duration {
            if seat1_connected {
                info!("Player 1 connected. Player 2 didn't");
                game_result.result = Some(AiArenaResult::Player2TimeOut);
                return Ok(game_result);
            } else if seat2_connected {
                info!("Player 2 connected. Player 1 didn't");
                game_result.result = Some(AiArenaResult::Player1TimeOut);
                return Ok(game_result);
            } else {
                return Err(Box::new(io::Error::new(
                    ErrorKind::TimedOut,
                    "None of the players connected within 30 seconds",
                )));
            }
        }
        interval.tick().await;
    }
}
