use axum::extract::{Path, State};
use axum::Json;
use common::api::errors::app_error::AppError;
use common::api::errors::process_error::ProcessError;
use common::api::state::AppState;
use common::models::{StartResponse, Status, TerminateResponse};
use common::paths;
use common::portpicker::pick_unused_port_in_range;
use common::utilities::directory::ensure_directory_structure;
use common::utilities::portpicker::Port;
use tempfile::TempDir;

#[tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/terminate/{process_key}",
    params(
        ("process_key" = u16, Path, description = "process_key SC2 instance to terminate")
    ),
    responses(
        (status = 200, description = "Request Completed", body = TerminateResponse)
    )
))]
pub async fn terminate_sc2(
    Path(port): Path<Port>,
    State(state): State<AppState>,
) -> Result<Json<TerminateResponse>, AppError> {
    if let Some((_, mut child)) = state.process_map.write().remove_entry(&port) {
        tracing::info!("Terminating SC2 on port {}", port);
        if let Err(e) = child.kill() {
            Err(ProcessError::TerminateError(e.to_string()).into())
        } else {
            let response = TerminateResponse {
                status: Status::Success,
            };

            Ok(Json(response))
        }
    } else {
        Err(ProcessError::TerminateError("Process Key entry does not exist".to_string()).into())
    }
}

#[tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger",utoipa::path(
    get,
    path = "/start",
    responses(
        (status = 200, description = "Request Completed", body = StartResponse)
    )
))]
pub async fn start_sc2(State(state): State<AppState>) -> Result<Json<StartResponse>, AppError> {
    let ws_port = pick_unused_port_in_range(9000..10000)
        .ok_or_else(|| ProcessError::Custom("Could not allocate port".to_string()))?;
    let tempdir = TempDir::new()
        .map_err(|e| ProcessError::Custom(format!("Could not create temp dir: {e:?}")))?;

    let log_dir = format!("{}/{}", "sc2_controller", ws_port);

    ensure_directory_structure(&state.settings.log_root, &log_dir)
        .await
        .map_err(|e| ProcessError::StartError(format!("{e:?}")))?;

    if let Ok(executable) = paths::executable() {
        let process_result = (async_process::Command::new(executable)
            .arg("-listen")
            .arg("0.0.0.0")
            .arg("-port")
            .arg(ws_port.to_string())
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
                state.process_map.write().insert(ws_port, process);
                let start_response = StartResponse {
                    status: Status::Success,
                    status_reason: "".to_string(),
                    port: ws_port,
                    process_key: ws_port,
                };
                Ok(Json(start_response))
            }
            Err(e) => Err(ProcessError::StartError(e.to_string()).into()),
        }
    } else {
        Err(ProcessError::StartError("Could not find executable".to_string()).into())
    }
}
