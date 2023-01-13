use crate::utils::move_bot_to_internal_dir;
use crate::PREFIX;
use common::api::errors::app_error::AppError;
use common::api::errors::download_error::DownloadError;
use common::api::errors::process_error::ProcessError;
use common::api::state::AppState;
use common::axum::body::StreamBody;
use common::axum::extract::{Path, State};
use common::axum::http::header;
use common::axum::Json;
use common::configuration::{get_proxy_host, get_proxy_port, get_proxy_url_from_env};
use common::models::bot_controller::{BotType, StartBot};
use common::models::{StartResponse, Status, TerminateResponse};
use common::procs::tcp_port::get_ipv4_port_for_pid;

use common::tokio::net::lookup_host;
use common::tokio_util::io::ReaderStream;
use common::utilities::directory::ensure_directory_structure;
use common::utilities::portpicker::Port;
use common::utilities::zip_utils::zip_directory;

use common::api::{BytesResponse, FileResponse};
use common::procs::create_stdout_and_stderr_files;
use common::reqwest::Client;
use common::tracing::log::error;
#[cfg(feature = "swagger")]
use common::utoipa;
use common::{tokio, tracing};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

#[common::tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
post,
path = "/terminate/{process_key}",
params(
("process_key" = u16, Path, description = "Key of process to terminate")
),
responses(
(status = 200, description = "Request Completed", body = TerminateResponse)
)
))]
pub async fn terminate_bot(
    Path(process_key): Path<Port>,
    State(state): State<AppState>,
) -> Result<Json<TerminateResponse>, AppError> {
    tracing::info!("Terminating bot with key {}", process_key);
    if let Some((_, mut child)) = state.process_map.write().remove_entry(&process_key) {
        if let Err(e) = child.kill() {
            return Err(ProcessError::TerminateError(e.to_string()).into());
        }
    } else {
        let message = format!("Bot {} entry does not exist", process_key);
        return Err(ProcessError::TerminateError(message).into());
    }

    Ok(Json(TerminateResponse {
        status: Status::Success,
    }))
}

#[common::tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
post,
path = "/start",
request_body = StartBot,
responses(
(status = 200, description = "Request Completed", body = StartResponse)
)
))]
pub async fn start_bot(
    State(state): State<AppState>,
    Json(start_bot): Json<StartBot>,
) -> Result<Json<StartResponse>, AppError> {
    let StartBot {
        bot_name,
        bot_type,
        opponent_id,
        player_num,
        match_id: _match_id,
        process_key,
        should_download,
    } = &start_bot;
    let bot_path =
        std::path::PathBuf::from(format!("{}/{}", &state.settings.bots_directory, bot_name));
    if *should_download {
        let proxy_url = get_proxy_url_from_env(PREFIX);
        let download_url = format!("http://{}/download_bot", proxy_url);

        let client = Client::new();
        let request = client
            .request(common::reqwest::Method::POST, &download_url)
            .json(player_num)
            .build()
            .map_err(|e| {
                AppError::Process(ProcessError::StartError(format!(
                    "Could not build download request: {:?}",
                    &e
                )))
            })?;

        let resp = match client.execute(request).await {
            Ok(resp) => resp,
            Err(err) => {
                error!("{:?}", err);
                return Err(ProcessError::StartError(format!(
                    "Could not download bot from url: {:?}",
                    &download_url
                ))
                .into());
            }
        };

        let status = resp.status();

        if status.is_client_error() || status.is_server_error() {
            return Err(ProcessError::StartError(format!(
                "Status: {:?}\nCould not download bot from url: {:?}",
                status, &download_url
            ))
            .into());
        }

        let bot_zip_bytes = resp
            .bytes()
            .await
            .map_err(|e| ProcessError::StartError(format!("{:?}", e)))?;

        if bot_path.exists() {
            let _ = tokio::fs::remove_dir(&bot_path).await;
            let _ = tokio::fs::remove_file(&bot_path).await;
        }
        // tokio::fs::write(&bot_path, bot_zip_bytes).await.map_err(DownloadError::from)?;
        common::utilities::zip_utils::zip_extract_from_memory(&bot_zip_bytes, &bot_path)
            .map_err(DownloadError::from)?;
    }
    let mut bot_path = format!("{}/{}", &state.settings.bots_directory, bot_name);

    let (program, filename) = match bot_type {
        BotType::CppWin32 => ("wine".to_string(), format!("{}.exe", bot_name)),
        BotType::CppLinux => (format!("./{}", bot_name), String::new()),
        BotType::DotnetCore => ("dotnet".to_string(), format!("{}.dll", bot_name)),
        BotType::Java => ("java".to_string(), format!("{}.jar", bot_name)),
        BotType::NodeJs => ("node".to_string(), format!("{}.js", bot_name)),
        BotType::Python => (state.settings.python.clone(), "run.py".to_string()),
    };

    if !std::path::Path::new(&bot_path).exists() {
        return Err(ProcessError::StartError(format!(
            "Supplied bot path does not exist: {:?}",
            &bot_path
        ))
        .into());
    }
    if state.settings.secure_mode {
        match move_bot_to_internal_dir(&state.settings, &bot_path, *player_num) {
            Ok(new_path) => {
                bot_path = new_path;
            }
            Err(e) => {
                let message = format!("Could not move bots to internal directory:\n{}", e);
                return Err(ProcessError::StartError(message).into());
            }
        }
    }
    if let Err(e) = ensure_directory_structure(&bot_path, "data").await {
        let message = format!("Could not validate directory structure:\n{}", e);
        return Err(ProcessError::StartError(message).into());
    }

    let log_file_path = std::path::Path::new(&bot_path)
        .join("data")
        .join("stderr.log");

    let (stdout_file, stderr_file) = match create_stdout_and_stderr_files(&log_file_path) {
        Ok(files) => files,
        Err(e) => {
            return Err(AppError::Process(ProcessError::StartError(format!(
                "Failed to create log files: {:?}",
                e.to_string()
            ))));
        }
    };

    let mut command = Command::new(program);

    if bot_type == &BotType::Java {
        command.arg("-jar");
    }
    if !filename.is_empty() {
        command.arg(filename);
    }
    let (proxy_host, proxy_port) = (get_proxy_host(PREFIX), get_proxy_port(PREFIX));

    let temp_proxy_host = format!("{}:{}", proxy_host, proxy_port);

    let resolved_proxy_host = match lookup_host(temp_proxy_host).await {
        Ok(mut addrs) => addrs.next().map(|x| x.ip().to_string()),
        Err(_) => None,
    }
    .unwrap_or(proxy_host);

    command
        .stdout(stdout_file)
        .stderr(stderr_file)
        .arg("--GamePort")
        .arg(&proxy_port)
        .arg("--LadderServer")
        .arg(resolved_proxy_host)
        .arg("--StartPort")
        .arg(&proxy_port)
        .arg("--OpponentId")
        .arg(opponent_id)
        .current_dir(&bot_path);

    tracing::debug!(
        "Starting bot with command {:?} {:?}",
        command.get_program(),
        command.get_args()
    );
    let mut process = match command.spawn() {
        Ok(mut process) => {
            tokio::time::sleep(Duration::from_secs(5)).await;
            match process.try_wait() {
                Ok(None) => {}
                Ok(Some(exit_status)) => {
                    let status_reason = format!(
                        "Bot {} has exited within 5 seconds with status {}",
                        bot_name, exit_status
                    );
                    return Ok(Json(StartResponse {
                        status: Status::Fail,
                        status_reason,
                        port: 0,
                        process_key: *process_key,
                    }));
                }
                Err(e) => {
                    let status_reason = format!("Error checking bot {} status: {}", bot_name, e);
                    return Ok(Json(StartResponse {
                        status: Status::Fail,
                        status_reason,
                        port: 0,
                        process_key: *process_key,
                    }));
                }
            }
            process
        }
        Err(e) => {
            return Err(ProcessError::StartError(e.to_string()).into());
        }
    };
    let pid = process.id();

    let max_retries = 10;
    let mut counter = 0;
    let mut port = None;
    #[cfg(target_os = "windows")]
    {
        let proxy_port: u16 = get_proxy_port(PREFIX).parse().unwrap();

        while port.is_none() && counter < max_retries {
            counter += 1;

            port = get_ipv4_port_for_pid(pid, proxy_port, true);
            if port.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let proxy_port: u16 = get_proxy_port(PREFIX).parse().unwrap();

        while port.is_none() && counter < max_retries {
            counter += 1;

            port = get_ipv4_port_for_pid(pid);
            if port.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
    let encoded_bot_name = common::urlencoding::encode(bot_name);

    match state.extra_info.write().entry(encoded_bot_name.to_string()) {
        Entry::Occupied(mut occ) => {
            occ.get_mut().insert("BotDirectory".to_string(), bot_path);
        }
        Entry::Vacant(vac) => {
            vac.insert(HashMap::new())
                .insert("BotDirectory".to_string(), bot_path);
        }
    }

    if let Some(port) = port {
        state.process_map.write().insert(port, process);

        let start_response = StartResponse {
            status: Status::Success,
            status_reason: "".to_string(),
            port,
            process_key: *process_key,
        };
        Ok(Json(start_response))
    } else {
        process.kill().expect("Could not kill process");
        Err(ProcessError::StartError("Could not find port for started process".to_string()).into())
    }
}

#[common::tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
get,
path = "/download/controller_log",
responses(
(status = 200, description = "Request Completed")
)
))]
pub async fn download_controller_log(
    State(state): State<AppState>,
) -> Result<FileResponse, AppError> {
    let log_path = format!(
        "{}/bot_controller/bot_controller.log",
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

#[common::tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
get,
path = "/download/bot/{process_key}/log",
params(
("process_key" = u16, Path, description = "process_key of bot process to fetch logs for")
),
responses(
(status = 200, description = "Request Completed", body = FileResponse)
)
))]
pub async fn download_bot_log(
    Path(bot_name): Path<String>,
    State(state): State<AppState>,
) -> Result<FileResponse, AppError> {
    let bot_directory = state
        .extra_info
        .read()
        .get(&bot_name)
        .and_then(|x| x.get("BotDirectory"))
        .ok_or_else(|| {
            AppError::Download(DownloadError::BotFolderNotFound(format!(
                "Could not find directory entry for bot_name {:?}",
                bot_name
            )))
        })?
        .clone();
    let log_path = std::path::Path::new(&bot_directory).join("data/stderr.log");

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

#[common::tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
get,
path = "/download/bot/{process_key}/data",
params(
("process_key" = u16, Path, description = "process_key of bot process to fetch data for")
),
responses(
(status = 200, description = "Request Completed", content_type = "application/octet")
)
))]
pub async fn download_bot_data(
    Path(bot_name): Path<String>,
    State(state): State<AppState>,
) -> Result<BytesResponse, AppError> {
    let bot_directory = state
        .extra_info
        .read()
        .get(&bot_name)
        .and_then(|x| x.get("BotDirectory"))
        .ok_or_else(|| {
            AppError::Download(DownloadError::BotFolderNotFound(format!(
                "Could not find directory entry for bot {:?}",
                bot_name
            )))
        })?
        .clone();
    let bot_data_directory = std::path::Path::new(&bot_directory).join("data");

    let buffer_size = bot_data_directory
        .metadata()
        .map(|x| x.len())
        .unwrap_or(65536);
    let mut buffer = std::io::Cursor::new(Vec::with_capacity(buffer_size as usize));

    zip_directory(&mut buffer, &bot_data_directory).map_err(DownloadError::from)?;
    let body = buffer.into_inner().into();
    let headers = [(header::CONTENT_TYPE, "application/zip; charset=utf-8")];

    Ok((headers, body))
}
