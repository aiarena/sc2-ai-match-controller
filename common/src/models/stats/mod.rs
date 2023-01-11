use serde::{Deserialize, Serialize};
use sysinfo::{CpuExt, PidExt, Process, ProcessExt, System, SystemExt};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;
#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct ProcessStats {
    pid: u32,
    name: String,
    cpu_usage: f32,
    memory: u64,
    virtual_memory: u64,
    cmd: Vec<String>,
    user_id: Option<String>,
    group_id: Option<String>,
    start_time: u64,
    run_time: u64,
}
impl ProcessStats {
    pub fn new(process: &Process) -> Self {
        Self {
            pid: process.pid().as_u32(),
            name: process.name().to_string(),
            cpu_usage: process.cpu_usage(),
            memory: process.memory(),
            virtual_memory: process.virtual_memory(),
            cmd: process.cmd().to_vec(),
            user_id: process.user_id().map(|x| x.to_string()),
            group_id: process.group_id().map(|x| x.to_string()),
            start_time: process.start_time(),
            run_time: process.run_time(),
        }
    }
}
impl Default for ProcessStats {
    fn default() -> Self {
        Self {
            pid: 0,
            name: "".to_string(),
            cpu_usage: 0.0,
            memory: 0,
            virtual_memory: 0,
            cmd: vec![],
            user_id: None,
            group_id: None,
            start_time: 0,
            run_time: 0,
        }
    }
}
#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Default, Debug, Deserialize, Serialize)]
pub struct HostStats {
    cpu_usage: f32,
    cpu_frequency: u64,
    available_memory: u64,
    used_memory: u64,
    free_memory: u64,
    total_memory: u64,
    uptime: u64,
}

impl HostStats {
    pub fn new(sys: &System) -> Self {
        Self {
            cpu_usage: sys.global_cpu_info().cpu_usage(),
            cpu_frequency: sys.global_cpu_info().frequency(),
            available_memory: sys.available_memory(),
            used_memory: sys.used_memory(),
            free_memory: sys.free_memory(),
            total_memory: sys.total_memory(),
            uptime: sys.uptime(),
        }
    }
}
