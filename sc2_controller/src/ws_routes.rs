use once_cell::sync::Lazy;
use std::io::ErrorKind::ConnectionRefused;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::WebSocketStream;

use crate::game::game_config::GameConfig;
use crate::game::player_result::PlayerResult;
use crate::game::sc2_result::Sc2Result;
use crate::player_seats::PlayerSeat;
use crate::websocket::player::PlayerError;
use axum::extract::ws::WebSocket;
use axum::extract::{ConnectInfo, State, WebSocketUpgrade};
use axum::response::IntoResponse;
use common::models::aiarena::aiarena_match::MatchRequest;
use common::PlayerNum;
use std::error::Error;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{debug, error, info, Instrument};

use crate::websocket::port_config::PortConfig;

static PORT_CONFIG: Lazy<Arc<RwLock<PortConfig>>> = Lazy::new(|| {
    Arc::new(RwLock::new(
        PortConfig::new().expect("Could not create port configuration"),
    ))
});

pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<Mutex<PlayerSeat>>>,
) -> impl IntoResponse {
    ws.max_message_size(128 << 20) // 128MiB
        .max_frame_size(32 << 20) // 32MiB
        .accept_unmasked_frames(true)
        .on_upgrade(move |socket| websocket(socket, state, addr))
}

#[tracing::instrument(skip(bot_ws, state), fields(bot_name))]
async fn websocket(bot_ws: WebSocket, state: Arc<Mutex<PlayerSeat>>, addr: SocketAddr) {
    let mut player_seat = state.lock().await;
    debug!(
        "Player seat connects {:?} to {:?}",
        addr, player_seat.internal_port
    );

    let tx = match player_seat.completion_tx.lock().unwrap().take() {
        Some(tx) => tx,
        None => return,
    };

    player_seat.channel.connect_player(bot_ws);

    let match_request = MatchRequest::read();
    debug!("Match Request: {:?}", match_request);

    let player_num = match player_seat.player_num {
        1 => PlayerNum::One,
        2 => PlayerNum::Two,
        _ => {
            error!("Invalid player number: {}", player_seat.player_num);
            let _ = tx.send(());
            return;
        }
    };

    let game_config = GameConfig::from_file(&match_request);
    let port_config = PORT_CONFIG.read().unwrap().clone();
    let pass_port = player_seat.pass_port;

    debug!("Starting Client Run");
    let p_result = match player_seat
        .channel
        .run(game_config, port_config, player_num, pass_port)
        .instrument(tracing::Span::current())
        .await
    {
        Ok(result) => result,
        Err(e) => match e {
            PlayerError::GameFault(_) => PlayerResult {
                game_loops: 0,
                frame_time: 0.0,
                player_id: 0,
                tags: indexmap::IndexSet::default(),
                result: Sc2Result::SC2Crash,
            },
            PlayerError::PlayerFault(_) => PlayerResult {
                game_loops: 0,
                frame_time: 0.0,
                player_id: 0,
                tags: indexmap::IndexSet::default(),
                result: Sc2Result::Crash,
            },
            PlayerError::PlayerTimeout(_) => PlayerResult {
                game_loops: 0,
                frame_time: 0.0,
                player_id: 0,
                tags: indexmap::IndexSet::default(),
                result: Sc2Result::Timeout,
            },
        },
    };

    info!(
        "Result for player {:?}: {:?}",
        player_seat.player_num, &p_result
    );

    if let Ok(mut game_result) = player_seat.game_result.write() {
        game_result.add_player_result(player_seat.player_num, p_result);
    }

    player_seat.channel.leave_game().await;

    tracing::info!("Done");
    let _ = tx.send(());
}

pub async fn connect(port: u16) -> Result<WebSocketStream<TcpStream>, Box<dyn Error>> {
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
        let socket =
            match tokio::time::timeout(Duration::from_secs(120), TcpStream::connect(&addr)).await {
                Ok(Ok(e)) => e,
                Ok(Err(ref e)) if e.kind() == ConnectionRefused => {
                    continue;
                }
                Ok(Err(e)) => return Err(Box::new(e)),
                Err(_) => return Err("Connection timeout".into()),
            };

        socket.set_nodelay(true)?;

        let (ws_stream, _) =
            tokio_tungstenite::client_async_with_config(url, socket, Some(config)).await?;

        return Ok(ws_stream);
    }

    Err("Websocket connection could not be formed".into())
}
