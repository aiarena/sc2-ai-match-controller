use axum::extract::{Path, State};
use axum::Json;
use parking_lot::RwLock;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::api::errors::app_error::AppError;
use crate::api::errors::process_error::ProcessError;
use crate::api::state::AppState;
use crate::models::stats::{HostStats, ProcessStats};
use crate::models::{ProcessStatusResponse, Status, TerminateResponse};
use crate::utilities::portpicker::Port;
use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Pid, PidExt, ProcessExt, ProcessStatus, RefreshKind, SystemExt};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

pub type ProcessMap = Arc<RwLock<HashMap<Port, async_process::Child>>>;

#[tracing::instrument]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/stats/host",
    responses(
        (status = 200, description = "Get host stats")
    )
))]
pub async fn stats_host() -> Result<Json<HostStats>, AppError> {
    let refresh_kind = RefreshKind::new()
        .with_cpu(CpuRefreshKind::everything())
        .with_memory();
    let sys = sysinfo::System::new_with_specifics(refresh_kind);
    let host_stats = HostStats::new(&sys);
    Ok(Json(host_stats))
}

#[tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/stats/{process_key}",
    params(
        ("process_key" = u16, Path, description = "process_key of process")
    ),
    responses(
        (status = 200, description = "Get procs stats for process")
    )
))]
pub async fn stats(
    Path(port): Path<Port>,
    State(state): State<AppState>,
) -> Result<Json<ProcessStats>, AppError> {
    let sys = sysinfo::System::new_all();
    if let Some(child) = state.process_map.read().get(&port) {
        let pid = Pid::from_u32(child.id());
        if let Some(process) = sys.process(pid) {
            let process_stats = ProcessStats::new(process);
            Ok(Json(process_stats))
        } else {
            state.process_map.write().remove(&port);
            Err(ProcessError::NotFound(child.id()).into())
        }
    } else {
        Err(ProcessError::NotInProcessMap(port).into())
    }
}

#[tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/status/{process_key}",
    params(
        ("process_key" = u16, Path, description = "process_key of process")
    ),
    responses(
        (status = 200, description = "Get status of process")
    )
))]
pub async fn status(
    Path(port): Path<Port>,
    State(state): State<AppState>,
) -> Result<Json<ProcessStatusResponse>, AppError> {
    let sys = sysinfo::System::new_all();
    if let Some(child) = state.process_map.read().get(&port) {
        let pid = Pid::from_u32(child.id());
        if let Some(process) = sys.process(pid) {
            let status = ProcStatus::from(process.status());

            Ok(Json(ProcessStatusResponse {
                running: status.is_running(),
                status,
            }))
        } else {
            Ok(Json(ProcessStatusResponse {
                running: false,
                status: ProcStatus::Dead,
            }))
        }
    } else {
        Err(ProcessError::NotInProcessMap(port).into())
    }
}

#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/terminate_all",
    responses(
        (status = 200, description = "Kill all processes")
    )
))]
pub async fn terminate_all(
    State(state): State<AppState>,
) -> Result<Json<TerminateResponse>, AppError> {
    let mut status = Status::Success;
    let mut temp_status_reason = String::new();

    for (process_key, mut child) in state.process_map.write().drain() {
        tracing::debug!("Terminating procs on port {}", process_key);
        let mut exited = false;
        for _ in 0..5 {
            if let Ok(status) = child.try_status() {
                if status.is_some() {
                    exited = true;
                    break;
                }
            }

            std::thread::sleep(Duration::from_secs(1));
        }
        if !exited {
            if let Err(e) = child.kill() {
                status = Status::Fail;
                let message = format!(
                    "{temp_status_reason}Failed to terminate process with key {process_key}:\n{e}\n",
                );
                temp_status_reason = message;
            }
        }
    }
    state.extra_info.write().clear();

    let response = TerminateResponse { status };
    if temp_status_reason.is_empty() {
        Ok(Json(response))
    } else {
        tracing::error!("{}", &temp_status_reason);
        Err(ProcessError::Custom(temp_status_reason).into())
    }
}

#[tracing::instrument(skip(state))]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/stats_all",
    responses(
        (status = 200, description = "Get procs stats for all processes ")
    )
))]
pub async fn stats_all(State(state): State<AppState>) -> Result<Json<Vec<ProcessStats>>, AppError> {
    let sys = sysinfo::System::new_all();
    let process_stats: Vec<ProcessStats> = state
        .process_map
        .read()
        .iter()
        .filter_map(|(_, child)| {
            let pid = Pid::from_u32(child.id());
            sys.process(pid).map(ProcessStats::new)
        })
        .collect();
    Ok(Json(process_stats))
}

#[tracing::instrument(skip(state))]
#[allow(dead_code)]
#[cfg_attr(feature = "swagger", utoipa::path(
    post,
    path = "/shutdown",
    responses(
        (status = 200, description = "Kill all processes and shutdown")
    )
))]
pub async fn shutdown(state: State<AppState>) -> Result<Json<TerminateResponse>, AppError> {
    tracing::info!("Shutdown request received. Terminating all running processes");
    let s = state.clone();
    terminate_all(s).await?;

    if let Err(e) = state.shutdown_sender.send(()).await {
        let message = format!("Could not send shutdown signal:\n{e}");
        Err(ProcessError::TerminateError(message).into())
    } else {
        tracing::info!("Shutting down...");
        let response = TerminateResponse {
            status: Status::Success,
        };
        Ok(Json(response))
    }
}
#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Copy, Clone)]
pub enum ProcStatus {
    Idle,
    Run,
    Sleep,
    Stop,
    Zombie,
    Tracing,
    Dead,
    Wakekill,
    Waking,
    Parked,
    LockBlocked,
    UninterruptibleDiskSleep,
    Unknown(u32),
}

impl ProcStatus {
    pub const fn is_running(&self) -> bool {
        match &self {
            ProcStatus::Run | ProcStatus::Idle => true,
            ProcStatus::Sleep => true,
            ProcStatus::Stop | ProcStatus::Zombie => false,
            ProcStatus::Tracing => true,
            ProcStatus::Dead => false,
            ProcStatus::Wakekill => false,
            ProcStatus::Waking => true,
            ProcStatus::Parked => true,
            ProcStatus::LockBlocked => true,
            ProcStatus::UninterruptibleDiskSleep => true,
            ProcStatus::Unknown(_) => true,
        }
    }
}

impl From<ProcessStatus> for ProcStatus {
    fn from(status: ProcessStatus) -> Self {
        match status {
            ProcessStatus::Idle => ProcStatus::Idle,
            ProcessStatus::Run => ProcStatus::Run,
            ProcessStatus::Sleep => ProcStatus::Sleep,
            ProcessStatus::Stop => ProcStatus::Stop,
            ProcessStatus::Zombie => ProcStatus::Zombie,
            ProcessStatus::Tracing => ProcStatus::Tracing,
            ProcessStatus::Dead => ProcStatus::Dead,
            ProcessStatus::Wakekill => ProcStatus::Wakekill,
            ProcessStatus::Waking => ProcStatus::Waking,
            ProcessStatus::Parked => ProcStatus::Parked,
            ProcessStatus::LockBlocked => ProcStatus::LockBlocked,
            ProcessStatus::UninterruptibleDiskSleep => ProcStatus::UninterruptibleDiskSleep,
            ProcessStatus::Unknown(x) => ProcStatus::Unknown(x),
        }
    }
}
