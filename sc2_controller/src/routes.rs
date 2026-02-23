use crate::player_seats::PlayerSeat;
use crate::ws_routes::websocket_handler;
use anyhow::{anyhow, Result};
use axum::routing::get;
use axum::Router;
use common::paths;
use common::portpicker::pick_unused_port_in_range;
use std::net::SocketAddr;
use std::str::FromStr;
use tempfile::TempDir;
use tokio::task::JoinHandle;

pub async fn open_player_seat(player_num: u8) -> Result<JoinHandle<()>> {
    // TODO: Use fixed ports instead
    let port = pick_unused_port_in_range(9000..10000)
        .ok_or_else(|| anyhow!("Could not allocate port".to_string()))?;

    let player_seat = PlayerSeat::new(player_num, port);

    start_sc2_process(&player_seat)
        .await
        .map_err(|e| anyhow!("Failed to start SC2 process: {e}"))?;

    let ws_server = start_ws_server(&player_seat)
        .await
        .map_err(|e| anyhow!("Failed to start WebSocket server: {e}"))?;

    Ok(ws_server)
}

async fn start_sc2_process(player_seat: &PlayerSeat) -> Result<()> {
    let tempdir = TempDir::new().map_err(|e| anyhow!("Could not create temp dir: {e:?}"))?;

    // TODO: Move to logging module
    let stdout_path = format!("/logs/stdout-{}.log", player_seat.external_port);
    let stdout_file = std::fs::File::create(&stdout_path)
        .map_err(|e| anyhow!("Could not create stdout file: {e:?}"))?;
    let stdout = async_process::Stdio::from(stdout_file);
    let stderr_path = format!("/logs/stderr-{}.log", player_seat.external_port);
    let stderr_file = std::fs::File::create(&stderr_path)
        .map_err(|e| anyhow!("Could not create stderr file: {e:?}"))?;
    let stderr = async_process::Stdio::from(stderr_file);

    if let Ok(executable) = paths::executable() {
        let process_result = (async_process::Command::new(executable)
            .stdout(stdout)
            .stderr(stderr)
            .arg("-listen")
            .arg("0.0.0.0")
            .arg("-port")
            .arg(player_seat.internal_port.to_string())
            .arg("-dataDir")
            .arg(paths::base_dir().to_str().unwrap())
            .arg("-displayMode")
            .arg("0")
            .arg("-tempDir")
            .arg(tempdir.path().to_str().unwrap())
            .current_dir(paths::cwd_dir()))
        .spawn();

        match process_result {
            Ok(_) => {
                tracing::info!(
                    "SC2 process for player seat {:?} started at port {:?}",
                    &player_seat.external_port,
                    &player_seat.internal_port
                );
                Ok(())
            }
            Err(e) => Err(anyhow!("Failed to start SC2 process: {e}")),
        }
    } else {
        Err(anyhow!("Could not find SC2 executable"))
    }
}

async fn start_ws_server(player_seat: &PlayerSeat) -> Result<JoinHandle<()>> {
    let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", player_seat.external_port)).unwrap();
    let app = Router::new()
        .route("/sc2api", get(websocket_handler))
        .with_state(player_seat.clone());
    let ws_server =
        axum::Server::bind(&addr).serve(app.into_make_service_with_connect_info::<SocketAddr>());
    let player_seat = player_seat.clone();

    let handle = tokio::spawn(async move {
        tracing::info!(
            "WebSocket server for player seat {:?} starting on {}",
            &player_seat.external_port,
            addr
        );
        if let Err(e) = ws_server.await {
            tracing::error!(
                "WebSocket server for player seat {:?} failed: {:?}",
                &player_seat.external_port,
                e
            );
        } else {
            tracing::info!(
                "WebSocket server for player seat {:?} shut down gracefully",
                &player_seat.external_port
            );
        }
    });

    tracing::info!(
        "WebSocket server for player seat {:?} opened on {}",
        &player_seat.external_port,
        addr
    );
    Ok(handle)
}
