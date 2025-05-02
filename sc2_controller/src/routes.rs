use axum::extract::State;
use axum::Json;
use common::api::errors::app_error::AppError;
use common::api::errors::process_error::ProcessError;
use common::api::state::AppState;
use common::models::{StartResponse, Status};
use common::paths;
use common::portpicker::pick_unused_port_in_range;
use tempfile::TempDir;

#[tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/start",
    responses(
        (status = 200, description = "Request Completed", body = StartResponse)
    )
))]
pub async fn start_sc2(State(state): State<AppState>) -> Result<Json<Vec<StartResponse>>, AppError> {

    // Terminate all previous SC2 processes
    for (port, mut child) in state.process_map.write().drain() {
        tracing::info!("Terminating SC2 on port {}", port);
        if let Err(e) = child.kill() {
            tracing::error!("Failed to terminate SC2 on port {}: {:?}", port, e);
        }
    }

    // Start two new SC2 processes
    match (start_process(&state).await, start_process(&state).await) {
        (Ok(response1), Ok(response2)) => {
            Ok(Json(vec![response1, response2]))
        }
        (Err(e), _) | (_, Err(e)) => {
            tracing::error!("Failed to start SC2: {:?}", e);
            Err(e)
        }
    }
}

async fn start_process(state: &AppState) -> Result<StartResponse, AppError> {
    let ws_port = pick_unused_port_in_range(9000..10000)
        .ok_or_else(|| ProcessError::Custom("Could not allocate port".to_string()))?;
    let tempdir = TempDir::new()
        .map_err(|e| ProcessError::Custom(format!("Could not create temp dir: {e:?}")))?;

    let stdout_path = format!("{}/sc2_controller/stdout-{}.log", &state.settings.log_root, ws_port);
    let stdout_file = std::fs::File::create(&stdout_path)
        .map_err(|e| ProcessError::Custom(format!("Could not create stdout file: {e:?}")))?;
    let stdout = async_process::Stdio::from(stdout_file);
    let stderr_path = format!("{}/sc2_controller/stderr-{}.log", &state.settings.log_root, ws_port);
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
                tracing::info!("SC2 listens on port {:?}", &ws_port);
                state.process_map.write().insert(ws_port, process);
                let start_response = StartResponse {
                    status: Status::Success,
                    status_reason: "".to_string(),
                    port: ws_port,
                    process_key: ws_port,
                };
                Ok(start_response)
            }
            Err(e) => Err(ProcessError::StartError(e.to_string()).into()),
        }
    } else {
        Err(ProcessError::StartError("Could not find executable".to_string()).into())
    }
}
