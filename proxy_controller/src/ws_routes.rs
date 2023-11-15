use std::io::ErrorKind::ConnectionRefused;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::WebSocketStream;

use crate::game::game_config::GameConfig;
use crate::game::player_result::PlayerResult;
use crate::game::sc2_result::Sc2Result;
use axum::extract::ws::WebSocket;
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use common::models::aiarena::aiarena_result::AiArenaResult;
use common::PlayerNum;
use parking_lot::RwLock;
use tokio::net::TcpStream;
use tokio::time::sleep;
use tracing::{debug, error, Instrument};

use crate::state::{ProxyState, SC2Url};
use crate::websocket::errors::player_error::PlayerError;
use crate::websocket::player::Player;
use crate::websocket::port_config::PortConfig;

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<RwLock<ProxyState>>>,
) -> impl IntoResponse {
    ws.max_message_size(128 << 20) // 128MiB
        .max_frame_size(32 << 20) // 32MiB
        .accept_unmasked_frames(true)
        .on_upgrade(move |socket| websocket(socket, state, addr))
}

#[tracing::instrument(skip(bot_ws, state), fields(bot_name))]
async fn websocket(bot_ws: WebSocket, state: Arc<RwLock<ProxyState>>, addr: SocketAddr) {
    debug!("Connection from {:?}", addr);
    state.write().add_client(addr);
    let settings = state.read().settings.clone();

    let sc2_url = state.write().get_free_sc2_url();
    debug!("Got free SC2 URL: {:?}", sc2_url);
    if sc2_url.is_none() {
        error!("No free SC2 ports available");
        if !state.read().game_result.as_ref().unwrap().has_any_result() {
            state.write().game_result.as_mut().unwrap().result = Some(AiArenaResult::Error);
        }

        return;
    }

    let sc2_url = sc2_url.unwrap();

    let sc2_ws = connect(&sc2_url).await;

    if sc2_ws.is_none() {
        error!("Could not connect to SC2");
        state.write().game_result.as_mut().unwrap().result = Some(AiArenaResult::Error);
        return;
    }

    let sc2_ws = sc2_ws.unwrap();
    let mut client_ws = Player::new(bot_ws, sc2_ws, addr);

    loop {
        let p_details = { state.read().get_player_details(addr) };

        if p_details.as_ref().and_then(|x| x.player_num()).is_none() {
            sleep(Duration::from_secs(3)).await;
        } else {
            break;
        }
    }

    let p_details = { state.read().get_player_details(addr) };
    debug!("Player Details: {:?}", p_details);

    let map = state.read().map.as_ref().map(|x| x.to_string()).unwrap();

    if let Some(bot_name) = p_details.as_ref().and_then(|x| x.bot_name()) {
        tracing::Span::current().record("bot_name", bot_name);
    }
    let player_num = p_details.as_ref().and_then(|x| x.player_num()).unwrap();

    if let PlayerNum::One = player_num {
        match client_ws.create_game(&map, settings.realtime).await {
            Ok(_) => {
                let mut s = state.write();
                debug!("Setting port_config and ready state");
                s.port_config = Some(PortConfig::new().unwrap());
                s.ready = true;
            }
            Err(e) => {
                error!("{:?}", e);
                //TODO: Initiate cleanup and early exit
                //TODO: Test invalid creategame
                state.write().game_result.as_mut().unwrap().result =
                    Some(AiArenaResult::InitializationError);
                return;
            }
        };
    }

    let max_counter = 200;
    let mut counter = 0;
    loop {
        debug!("Waiting for state to become ready");
        counter += 1;
        let ready = { state.read().ready };
        if ready || counter > max_counter {
            break;
        } else {
            sleep(Duration::from_millis(250)).await;
        }
    }
    if state.read().ready {
        let current_match = state.read().current_match.clone().unwrap();

        let ac_config = state.read().settings.clone();
        let game_config = GameConfig::new(&current_match, &ac_config);
        state.write().game_config = Some(game_config.clone());
        let port_config = state.read().port_config.clone().unwrap();
        debug!("Starting Client Run");
        let p_result = match client_ws
            .run(game_config, port_config, player_num)
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
                        state.write().game_result.as_mut().unwrap().set_error();
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
                        state.write().game_result.as_mut().unwrap().set_error();
                    }
                    PlayerError::UnexpectedRequest(e) => {
                        error!("{:?}", e);
                        state.write().game_result.as_mut().unwrap().set_init_error();
                    }
                    PlayerError::ProtoParseError(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::SC2Crash;
                        state.write().game_result.as_mut().unwrap().set_error();
                    }
                    PlayerError::CreateGame(e) => {
                        error!("{:?}", e);
                        state.write().game_result.as_mut().unwrap().set_init_error();
                    }
                    PlayerError::JoinGameTimeout(e) => {
                        error!("{:?}", e);
                        state.write().game_result.as_mut().unwrap().set_init_error();
                    }
                    PlayerError::Sc2Timeout(e) => {
                        error!("{:?}", e);
                        // If the game completion was forced (timeout or crash), the other bot might get a timeout
                        // from sc2. Check if there is a result before erroring the match
                        if !state.read().game_result.as_ref().unwrap().has_any_result() {
                            state.write().game_result.as_mut().unwrap().set_error();
                        }
                    }
                    PlayerError::BotTimeout(e) => {
                        error!("{:?}", e);
                        temp_result = Sc2Result::Timeout;
                        state.write().game_result.as_mut().unwrap().set_error();
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
        state
            .write()
            .game_result
            .as_mut()
            .unwrap()
            .add_player_result(player_num, p_result);
    } else {
        error!("Timeout while waiting for game to become ready");
        state.write().game_result.as_mut().unwrap().result =
            Some(AiArenaResult::InitializationError);
        return;
    }
    tracing::info!("Done");
}

pub async fn connect(sc2_url: &SC2Url) -> Option<WebSocketStream<TcpStream>> {
    let url = format!("ws://{}:{}/sc2api", sc2_url.host, sc2_url.port);
    let addr = format!("{}:{}", sc2_url.host, sc2_url.port);

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
