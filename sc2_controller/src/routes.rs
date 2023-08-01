use axum::body::StreamBody;
use axum::extract::{Path, State};
use axum::http::header;
use axum::Json;
use common::api::errors::app_error::AppError;
use common::api::errors::download_error::DownloadError;
use common::api::errors::map_error::MapError;
use common::api::errors::process_error::ProcessError;
use common::api::state::AppState;
use common::configuration::get_proxy_url_from_env;
use common::models::bot_controller::MapData;
use common::models::{StartResponse, Status, TerminateResponse};
use common::paths;
use common::portpicker::pick_unused_port_in_range;
use common::utilities::directory::ensure_directory_structure;
use common::utilities::portpicker::Port;
use reqwest::header::HeaderName;
use reqwest::Client;
use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use tracing::info;

use crate::PREFIX;

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
pub async fn start_sc2(
    State(state): State<AppState>,
    Json(map_name): Json<String>,
) -> Result<Json<StartResponse>, AppError> {
    let map_name = map_name.replace(".SC2Map", "");
    let map_path = paths::base_dir()
        .join("maps")
        .join(format!("{map_name}.SC2Map"));
    if !map_path.exists() {
        let proxy_url = get_proxy_url_from_env(PREFIX);
        let download_url = format!("http://{proxy_url}/download_map");

        let client = Client::new();
        let request = client
            .request(reqwest::Method::GET, &download_url)
            .build()
            .map_err(|e| {
                AppError::Process(ProcessError::StartError(format!(
                    "Could not build download request: {:?}",
                    &e
                )))
            })?;
        info!("Downloading map {}", map_name);
        let resp = match client.execute(request).await {
            Ok(resp) => resp,
            Err(err) => {
                tracing::error!("{:?}", err);
                return Err(ProcessError::StartError(format!(
                    "Could not download map from url: {:?}",
                    &download_url
                ))
                .into());
            }
        };

        let status = resp.status();

        if status.is_client_error() || status.is_server_error() {
            return Err(ProcessError::StartError(format!(
                "Status: {:?}\nCould not download map from url: {:?}",
                status, &download_url
            ))
            .into());
        }

        let map_bytes = resp
            .bytes()
            .await
            .map_err(|e| ProcessError::StartError(format!("{e:?}")))?;

        let mut file = tokio::fs::File::create(map_path)
            .await
            .map_err(|err| ProcessError::Custom(format!("Could not download map: {err:?}")))?;
        file.write_all(&map_bytes)
            .await
            .map_err(|err| ProcessError::Custom(format!("Could not write map to disk: {err:?}")))?;
    }

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

pub async fn download_controller_log(
    State(state): State<AppState>,
) -> Result<
    (
        [(HeaderName, &'static str); 1],
        StreamBody<ReaderStream<tokio::fs::File>>,
    ),
    AppError,
> {
    let log_path = format!(
        "{}/sc2_controller/sc2_controller.log",
        &state.settings.log_root
    );
    let file = tokio::fs::File::open(&log_path)
        .await
        .map_err(|e| AppError::Download(DownloadError::FileNotFound(e)))?;
    // convert the `AsyncRead` into a `Stream`
    let stream = ReaderStream::new(file);
    // convert the `Stream` into an `axum::body::HttpBody`
    let body = StreamBody::new(stream);

    let headers = [(header::CONTENT_TYPE, "text/log; charset=utf-8")];
    Ok((headers, body))
}

#[tracing::instrument]
#[cfg_attr(feature = "swagger",utoipa::path(
get,
path = "/find_map",
responses(
(status = 200, description = "Map Found", body = StartResponse)
)
))]
pub async fn find_map(Path(map_name): Path<String>) -> Result<Json<MapData>, AppError> {
    common::paths::maps::find_map(&map_name)
        .map_err(|err| MapError::from(err).into())
        .map(|map_path| {
            Json(MapData {
                query: map_name,
                map_path,
            })
        })
}
