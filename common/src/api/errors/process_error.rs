use serde::Deserialize;

use crate::utilities::portpicker::Port;

/// Errors that can happen interacting with processes.
#[derive(Debug, Deserialize)]
pub enum ProcessError {
    NotFound(u32),
    NotInProcessMap(Port),
    Custom(String),
    StartError(String),
    TerminateError(String),
}
