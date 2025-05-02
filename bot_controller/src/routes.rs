use crate::PREFIX;
use axum::extract::State;
use axum::Json;
use common::api::errors::app_error::AppError;
use common::api::errors::process_error::ProcessError;
use common::api::state::AppState;
use common::configuration::{get_proxy_host, get_proxy_port};
use common::models::bot_controller::{BotType, StartBot};
use common::models::{StartResponse, Status};
use common::procs::tcp_port::get_ipv4_port_for_pid;
use common::PlayerNum;

use common::utilities::directory::ensure_directory_structure;
use tokio::net::lookup_host;
use tracing::debug;

use common::procs::create_stdout_and_stderr_files;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::time::Duration;

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
    
    // Terminate all previous bot processes
    for (port, mut child) in state.process_map.write().drain() {
        tracing::info!("Terminating bot process {}", port);
        if let Err(e) = child.kill() {
            tracing::error!("Failed to terminate bot process {}: {:?}", port, e);
        }
    }

    let StartBot {
        bot_name,
        bot_type,
        opponent_id,
        player_num,
        match_id: _match_id,
        process_key,
    } = &start_bot;

    let bot_num = match player_num {
        PlayerNum::One => 1,
        PlayerNum::Two => 2,
    };
    let bot_path = format!(
        "{}/bot{}/{}",
        &state.settings.bot_directory, bot_num, bot_name
    );

    let (program, filename) = match bot_type {
        BotType::CppWin32 => ("wine".to_string(), format!("{bot_name}.exe")),
        BotType::CppLinux => (format!("./{bot_name}"), String::new()),
        BotType::DotnetCore => ("dotnet".to_string(), format!("{bot_name}.dll")),
        BotType::Java => ("java".to_string(), format!("{bot_name}.jar")),
        BotType::NodeJs => ("node".to_string(), format!("{bot_name}.js")),
        BotType::Python => (state.settings.python.clone(), "run.py".to_string()),
    };

    if !std::path::Path::new(&bot_path).exists() {
        return Err(ProcessError::StartError(format!(
            "Supplied bot path does not exist: {:?}",
            &bot_path
        ))
        .into());
    }
    if let Err(e) = ensure_directory_structure(&bot_path, "data").await {
        let message = format!("Could not validate directory structure:\n{e}");
        return Err(ProcessError::StartError(message).into());
    }
    if !std::path::Path::new(&state.settings.log_root).exists() {
        if let Err(e) = tokio::fs::create_dir_all(&state.settings.log_root).await {
            return Err(ProcessError::StartError(e.to_string()).into());
        }
    }
    if let Err(e) = ensure_directory_structure(&state.settings.log_root, bot_name).await {
        let message = format!("Could not validate directory structure:\n{e}");
        return Err(ProcessError::StartError(message).into());
    }
    debug!("Bot log dir exists");

    let log_file_path = std::path::PathBuf::from(&bot_path)
        .join("logs")
        .join("stderr.log");
    debug!("Bot log path: {:?}", log_file_path);

    let (stdout_file, stderr_file) = match create_stdout_and_stderr_files(&log_file_path) {
        Ok(files) => files,
        Err(e) => {
            return Err(AppError::Process(ProcessError::StartError(format!(
                "Failed to create log files: {:?}",
                e.to_string()
            ))));
        }
    };

    debug!("Log files created: {:?}", log_file_path);

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

    let mut command = async_process::Command::new(&program);

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
                let _ = std::fs::set_permissions(&bot_file_path, perms);
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

    let temp_proxy_host = format!("{proxy_host}:{proxy_port}");

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

    debug!("Starting bot with command {:?}", &command);
    let mut process = match command.spawn() {
        Ok(mut process) => {
            tokio::time::sleep(Duration::from_secs(2)).await;
            match process.try_status() {
                Ok(None) => {}
                Ok(Some(exit_status)) => {
                    return Err(ProcessError::StartError(format!(
                        "Bot {bot_name} has exited within 5 seconds with status {exit_status}"
                    ))
                    .into());
                }
                Err(e) => {
                    return Err(ProcessError::StartError(format!(
                        "Error checking bot {bot_name} status: {e}"
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
