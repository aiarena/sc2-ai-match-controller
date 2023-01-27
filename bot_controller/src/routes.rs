use crate::utils::move_bot_to_internal_dir;
use crate::PREFIX;
use axum::body::StreamBody;
use axum::extract::{Path, State};
use axum::http::header;
use axum::Json;
use common::api::errors::app_error::AppError;
use common::api::errors::download_error::DownloadError;
use common::api::errors::process_error::ProcessError;
use common::api::state::AppState;
use common::configuration::{get_proxy_host, get_proxy_port, get_proxy_url_from_env};
use common::models::bot_controller::{BotType, PlayerNum, StartBot};
use common::models::{StartResponse, Status, TerminateResponse};
use common::procs::tcp_port::get_ipv4_port_for_pid;

use common::utilities::directory::ensure_directory_structure;
use common::utilities::portpicker::Port;
use common::utilities::zip_utils::zip_directory;
use tokio::net::lookup_host;
use tokio_util::io::ReaderStream;
use tracing::debug;

use common::api::{BytesResponse, FileResponse};
use common::procs::create_stdout_and_stderr_files;
use reqwest::{Client, StatusCode};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;
use tracing::log::error;

#[tracing::instrument(skip(state))]
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

#[tracing::instrument(skip(state))]
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
        let bot_download_url = format!("http://{}/download_bot", proxy_url);

        download_and_extract(&bot_download_url, &bot_path, player_num).await?;

        let bot_data_download_url = format!("http://{}/download_bot_data", proxy_url);
        let bot_data_path = bot_path.join("data");
        match download_and_extract(&bot_data_download_url, &bot_data_path, player_num).await {
            Ok(_) => {}
            Err(AppError::Download(DownloadError::NotAvailable(e))) => {
                debug!("{:?}", e)
            }
            Err(e) => return Err(e),
        }
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
    if !std::path::Path::new(&state.settings.log_root).exists() {
        if let Err(e) = tokio::fs::create_dir_all(&state.settings.log_root).await {
            return Err(ProcessError::StartError(e.to_string()).into());
        }
    }
    if let Err(e) = ensure_directory_structure(&state.settings.log_root, bot_name).await {
        let message = format!("Could not validate directory structure:\n{}", e);
        return Err(ProcessError::StartError(message).into());
    }

    let log_file_path = std::path::Path::new(&state.settings.log_root)
        .join(bot_name)
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

    let encoded_bot_name = urlencoding::encode(bot_name);

    match state.extra_info.write().entry(encoded_bot_name.to_string()) {
        Entry::Occupied(mut occ) => {
            occ.get_mut()
                .insert("BotDirectory".to_string(), bot_path.clone());
        }
        Entry::Vacant(vac) => {
            vac.insert(HashMap::new())
                .insert("BotDirectory".to_string(), bot_path.clone());
        }
    }

    let mut command = Command::new(&program);

    if bot_type == &BotType::Java {
        command.arg("-jar");
    }
    if !filename.is_empty() {
        command.arg(filename);
    }

    #[cfg(not(windows))]
    {
        use std::os::unix::fs::PermissionsExt;
        use tracing::debug;
        if bot_type == &BotType::CppLinux {
            let bot_file_path = std::path::Path::new(&bot_path).join(&program);
            if let Ok(bot_file) = std::fs::metadata(&bot_file_path) {
                debug!("Setting bot file permissions");
                let mut perms = bot_file.permissions();
                perms.set_mode(0o777);
                std::fs::set_permissions(&bot_file_path, perms);
            }
            // if std::path::Path::new(&bot_path).is_dir() {
            //     for item in std::fs::read_dir(&bot_path).unwrap() {
            //         debug!("Setting bot folder item's permissions");
            //         if let Ok(item) = item {
            //             if let Ok(bot_dir_file) = std::fs::metadata(&item.path()) {
            //                 let mut perms = bot_dir_file.permissions();
            //                 perms.set_mode(0o777);
            //                 perms.set_mode(mode)
            //             }
            //         }
            //     }
            // }
        }
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
                    return Err(ProcessError::StartError(format!(
                        "Bot {} has exited within 5 seconds with status {}",
                        bot_name, exit_status
                    ))
                    .into());
                }
                Err(e) => {
                    return Err(ProcessError::StartError(format!(
                        "Error checking bot {} status: {}",
                        bot_name, e
                    ))
                    .into());
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

    while port.is_none() && counter < max_retries {
        counter += 1;

        port = get_ipv4_port_for_pid(pid);
        if port.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
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

async fn download_and_extract(
    url: &str,
    path: &std::path::Path,
    player_num: &PlayerNum,
) -> Result<(), AppError> {
    let client = Client::new();
    let request = client
        .request(reqwest::Method::POST, url)
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
                &url
            ))
            .into());
        }
    };

    let status = resp.status();

    if status.is_client_error() || status.is_server_error() {
        let text = resp.text().await.unwrap_or_else(|_| "Error".to_string());
        if status == StatusCode::NOT_IMPLEMENTED {
            return Err(AppError::Download(DownloadError::NotAvailable(text)));
        } else {
            return Err(ProcessError::StartError(format!(
                "Status: {:?}\nCould not download bot from url: {:?}",
                status, &url
            ))
            .into());
        }
    }

    let bot_zip_bytes = resp
        .bytes()
        .await
        .map_err(|e| ProcessError::StartError(format!("{:?}", e)))?;

    if path.exists() {
        let _ = tokio::fs::remove_dir(&path).await;
        let _ = tokio::fs::remove_file(&path).await;
    }
    // tokio::fs::write(&bot_path, bot_zip_bytes).await.map_err(DownloadError::from)?;
    common::utilities::zip_utils::zip_extract_from_memory(&bot_zip_bytes, &path.to_path_buf())
        .map_err(DownloadError::from)
        .map_err(AppError::from)
}

#[tracing::instrument(skip(state))]
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

#[tracing::instrument(skip(state))]
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
    let log_path = std::path::Path::new(&state.settings.log_root)
        .join(&bot_name)
        .join("stderr.log");

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

#[tracing::instrument(skip(state))]
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
