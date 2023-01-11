use crate::api::process::ProcStatus;
use crate::utilities::portpicker::Port;
use serde::{Deserialize, Serialize};
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

pub mod bot_controller;
pub mod proxy_controller;
pub mod sc2_controller;
pub mod stats;

#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub enum Status {
    Success,
    Fail,
}
#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct TerminateResponse {
    pub status: Status,
}
#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Debug, Deserialize, Serialize)]
pub struct StartResponse {
    pub status: Status,
    pub status_reason: String,
    pub port: Port,
    pub process_key: Port,
}
#[cfg_attr(feature = "swagger", derive(ToSchema))]
#[derive(Serialize, Deserialize, Copy, Clone)]
pub struct ProcessStatusResponse {
    pub(crate) running: bool,
    pub(crate) status: ProcStatus,
}
