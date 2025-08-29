use once_cell::sync::Lazy;
use std::io::ErrorKind::ConnectionRefused;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::WebSocketStream;

use crate::game::game_config::GameConfig;
use crate::game::game_result::GAME_RESULT;
use crate::game::player_result::PlayerResult;
use crate::game::sc2_result::Sc2Result;
use crate::player_seats::PlayerSeat;
use axum::extract::ws::WebSocket;
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use common::models::aiarena::aiarena_game_result::AiArenaGameResult;
use common::models::aiarena::aiarena_match::MatchRequest;
use common::PlayerNum;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tracing::{debug, error, info, Instrument};

use crate::websocket::errors::player_error::PlayerError;
use crate::websocket::player::Player;
use crate::websocket::port_config::PortConfig;

struct GameReadyFlag {
    pub ready: bool,
}

static GAME_READY_FLAG: Lazy<Arc<RwLock<GameReadyFlag>>> =
    Lazy::new(|| Arc::new(RwLock::new(GameReadyFlag { ready: false })));

static PORT_CONFIG: Lazy<Arc<RwLock<PortConfig>>> = Lazy::new(|| {
    Arc::new(RwLock::new(
        PortConfig::new().expect("Could not create port configuration"),
    ))
});

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<PlayerSeat>,
) -> impl IntoResponse {
    ws.max_message_size(128 << 20) // 128MiB
        .max_frame_size(32 << 20) // 32MiB
        .accept_unmasked_frames(true)
        .on_upgrade(move |socket| websocket(socket, state, addr))
}

#[tracing::instrument(skip(bot_ws, player_seat), fields(bot_name))]
async fn websocket(bot_ws: WebSocket, player_seat: PlayerSeat, addr: SocketAddr) {
    debug!("Player seat connects {:?} to {:?}", addr, player_seat.internal_port);
    let settings = player_seat.settings.clone();

    let match_request = MatchRequest::read();
    debug!("Match Request: {:?}", match_request);
    let match_id = match_request.match_id;
    GAME_RESULT.write().unwrap().set(match_id);

    let sc2_ws = connect(player_seat.internal_port).await;

    if sc2_ws.is_none() {
        error!("Could not connect to SC2");
        GAME_RESULT.write().unwrap().set_error(match_id);
        return store_game_result(match_id);
    }

    let sc2_ws = sc2_ws.unwrap();
    let mut client_ws = Player::new(bot_ws, sc2_ws);

    let map = match_request.map_name.clone();

    let player_num = match player_seat.player_num {
        1 => PlayerNum::One,
        2 => PlayerNum::Two,
        _ => {
            error!("Invalid player number: {}", player_seat.player_num);
            GAME_RESULT.write().unwrap().set_init_error(match_id);
            return store_game_result(match_id);
        }
    };

    if let PlayerNum::One = player_num {
        match client_ws.create_game(&map, settings.realtime).await {
            Ok(_) => {
                let mut s = GAME_READY_FLAG.write().unwrap();
                debug!("Setting port_config and ready state");
                s.ready = true;
            }
            Err(e) => {
                error!("{:?}", e);
                //TODO: Initiate cleanup and early exit
                //TODO: Test invalid creategame
                GAME_RESULT.write().unwrap().set_init_error(match_id);
                return store_game_result(match_id);
            }
        };
    }

    let max_counter = 200;
    let mut counter = 0;
    loop {
        debug!("Waiting for state to become ready");
        counter += 1;
        let ready = { GAME_READY_FLAG.read().unwrap().ready };
        if ready || counter > max_counter {
            break;
        } else {
            sleep(Duration::from_millis(250)).await;
        }
    }

    let game_config = GameConfig::from_file(&settings, &match_request);
    let port_config = PORT_CONFIG.read().unwrap().clone();

    if counter <= max_counter {
        debug!("Starting Client Run");
        let p_result = match client_ws
            .run(game_config, port_config, player_num, player_seat.pass_port)
            .instrument(tracing::Span::current())
            .await
        {
            Ok(result) => result,
            Err(e) => {
                let mut temp_result = Sc2Result::SC2Crash;
                error!("{:?}", e);
                match e {
                    PlayerError::BotQuit => temp_result = Sc2Result::Defeat,
                    PlayerError::NoMessageAvailable => {
                        temp_result = Sc2Result::SC2Crash;
                        GAME_RESULT.write().unwrap().set_error(match_id);
                    }
                    PlayerError::BotWebsocket(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::Crash;
                    }
                    PlayerError::Sc2Websocket(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::SC2Crash;
                    }
                    PlayerError::BotUnexpectedMessage(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::Crash;
                    }
                    PlayerError::Sc2UnexpectedMessage(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::SC2Crash;
                        GAME_RESULT.write().unwrap().set_error(match_id);
                    }
                    PlayerError::UnexpectedRequest(e) => {
                        error!("{:?}", e);
                        GAME_RESULT.write().unwrap().set_init_error(match_id);
                    }
                    PlayerError::ProtoParseError(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::SC2Crash;
                        GAME_RESULT.write().unwrap().set_error(match_id);
                    }
                    PlayerError::CreateGame(e) => {
                        error!("{:?}", e);
                        GAME_RESULT.write().unwrap().set_init_error(match_id);
                    }
                    PlayerError::JoinGameTimeout(e) => {
                        error!("{:?}", e);
                        GAME_RESULT.write().unwrap().set_init_error(match_id);
                    }
                    PlayerError::Sc2Timeout(e) => {
                        error!("{:?}", e);
                        // If the game completion was forced (timeout or crash), the other bot might get a timeout
                        // from sc2. Check if there is a result before erroring the match

                        if !GAME_RESULT.read().unwrap().has_any_result() {
                            GAME_RESULT.write().unwrap().set_init_error(match_id);
                        }
                    }
                    PlayerError::BotTimeout(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::Timeout;
                        GAME_RESULT.write().unwrap().set_error(match_id);
                    }
                }
                PlayerResult {
                    game_loops: 0,
                    frame_time: 0.0,
                    player_id: 0,
                    tags: indexmap::IndexSet::default(),
                    result: temp_result,
                }
            }
        };
        debug!("{:?}", &p_result);
        GAME_RESULT
            .write()
            .unwrap()
            .add_player_result(match_id, player_num, p_result);
    } else {
        error!("Timeout while waiting for game to become ready");
        GAME_RESULT.write().unwrap().set_init_error(match_id);
        return store_game_result(match_id);
    }
    tracing::info!("Done");
    return store_game_result(match_id);
}

pub async fn connect(port: u16) -> Option<WebSocketStream<TcpStream>> {
    let url = format!("ws://127.0.0.1:{}/sc2api", port);
    let addr = format!("127.0.0.1:{}", port);

    debug!("Connecting to the SC2 process: {:?}, {:?}", url, addr);

    let config = WebSocketConfig {
        max_message_size: Some(128 << 20),
        max_frame_size: Some(32 << 20),
        accept_unmasked_frames: true,
        ..Default::default()
    };

    for _ in 0..60 {
        sleep(Duration::new(1, 0)).await;
        let socket = match tokio::time::timeout(Duration::from_secs(120), TcpStream::connect(&addr))
            .await
            .ok()?
        {
            Ok(e) => e,
            Err(ref e) if e.kind() == ConnectionRefused => {
                continue;
            }
            Err(e) => panic!("E: {e:?}"),
        };

        socket.set_nodelay(true).unwrap();

        let (ws_stream, _) = tokio_tungstenite::client_async_with_config(url, socket, Some(config))
            .await
            .expect("Failed to connect");

        return Some(ws_stream);
    }

    error!("Websocket connection could not be formed");
    None
}

/// Store the game result on disk.
/// The input match id ensures that thread that process previous matches don't overwrite the current match result.
fn store_game_result(match_id: u32) {
    let game_result = GAME_RESULT.read().unwrap().clone();

    if game_result.match_id == match_id {
        if game_result.is_ready() {
            let aiarena_game_result = AiArenaGameResult::from(&game_result);

            info!("Game result: {:?}", &aiarena_game_result);

            aiarena_game_result.to_json_file();

            info!("Game result stored successfully");
        } else {
            info!("Waiting for results from both players before storing the game result");
        }
    } else {
        info!(
            "Ignoring game result for match {} as current match is {}",
            match_id, game_result.match_id
        );
    }
}
