use crate::game::game_result::GAME_RESULT;
use crate::player_seats::PlayerSeat;
use crate::ws_routes::websocket_handler;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use common::api::errors::app_error::AppError;
use common::api::errors::process_error::ProcessError;
use common::api::state::AppState;
use common::models::Status;
use common::paths;
use common::portpicker::pick_unused_port_in_range;
use std::net::SocketAddr;
use std::str::FromStr;
use tempfile::TempDir;

#[tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/start",
    responses(
        (status = 200, description = "Request Completed", body = Status)
    )
))]
pub async fn start_sc2(State(state): State<AppState>) -> Result<Json<Status>, AppError> {
    // Shutdown all WebSocket servers
    for shutdown_tx in state.ws_shutdown_senders.write().drain(..) {
        let _ = shutdown_tx.send(());
    }

    // Terminate all previous SC2 processes
    for (port, mut child) in state.process_map.write().drain() {
        tracing::info!("Terminating SC2 on port {}", port);
        if let Err(e) = child.kill() {
            tracing::error!("Failed to terminate SC2 on port {}: {:?}", port, e);
        }
    }

    // Delete previous match result from memory
    GAME_RESULT.write().unwrap().reset();

    // Start two new SC2 processes
    match (
        open_player_seat(&state, 1).await,
        open_player_seat(&state, 2).await,
    ) {
        (Ok(_), Ok(_)) => Ok(Json(Status::Success)),
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!("Failed to start SC2: {:?}", e);
            Err(e)
        }
    }
}

async fn open_player_seat(state: &AppState, player_num: u8) -> Result<Status, AppError> {
    let port = pick_unused_port_in_range(9000..10000)
        .ok_or_else(|| ProcessError::Custom("Could not allocate port".to_string()))?;

    let player_seat = PlayerSeat::new(state.settings.clone(), player_num, port);

    match start_sc2_process(state, &player_seat).await {
        Ok(_) => match start_ws_server(state, &player_seat).await {
            Ok(_) => Ok(Status::Success),
            Err(e) => Err(e),
        },
        Err(e) => Err(e),
    }
}

async fn start_sc2_process(state: &AppState, player_seat: &PlayerSeat) -> Result<Status, AppError> {
    let tempdir = TempDir::new()
        .map_err(|e| ProcessError::Custom(format!("Could not create temp dir: {e:?}")))?;

    let stdout_path = format!(
        "{}/sc2_controller/stdout-{}.log",
        &state.settings.log_root, player_seat.external_port
    );
    let stdout_file = std::fs::File::create(&stdout_path)
        .map_err(|e| ProcessError::Custom(format!("Could not create stdout file: {e:?}")))?;
    let stdout = async_process::Stdio::from(stdout_file);
    let stderr_path = format!(
        "{}/sc2_controller/stderr-{}.log",
        &state.settings.log_root, player_seat.external_port
    );
    let stderr_file = std::fs::File::create(&stderr_path)
        .map_err(|e| ProcessError::Custom(format!("Could not create stderr file: {e:?}")))?;
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
            Ok(process) => {
                tracing::info!(
                    "SC2 process for player seat {:?} started at port {:?}",
                    &player_seat.external_port,
                    &player_seat.internal_port
                );
                state
                    .process_map
                    .write()
                    .insert(player_seat.internal_port, process);

                Ok(Status::Success)
            }
            Err(e) => Err(ProcessError::StartError(e.to_string()).into()),
        }
    } else {
        Err(ProcessError::StartError("Could not find SC2 executable".to_string()).into())
    }
}

async fn start_ws_server(state: &AppState, player_seat: &PlayerSeat) -> Result<Status, AppError> {
    let addr = SocketAddr::from_str(&format!("0.0.0.0:{}", player_seat.external_port)).unwrap();
    let app = Router::new()
        .route("/sc2api", get(websocket_handler))
        .with_state(player_seat.clone());
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let ws_server = axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            shutdown_rx.await.ok();
        });
    let player_seat = player_seat.clone();

    state.ws_shutdown_senders.write().push(shutdown_tx);
    tokio::spawn(async move {
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
    Ok(Status::Success)
}
