use crate::api::errors::download_error::DownloadError;
use crate::api::errors::map_error::MapError;
use crate::api::errors::process_error::ProcessError;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum AppError {
    Process(ProcessError),
    Map(MapError),
    Download(DownloadError),
}

impl From<ProcessError> for AppError {
    fn from(inner: ProcessError) -> Self {
        Self::Process(inner)
    }
}

impl From<MapError> for AppError {
    fn from(inner: MapError) -> Self {
        Self::Map(inner)
    }
}

impl From<DownloadError> for AppError {
    fn from(inner: DownloadError) -> Self {
        Self::Download(inner)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Self::Process(ProcessError::NotFound(pid)) => {
                let message = format!("Process for PID {} could not be found.", pid);
                (StatusCode::INTERNAL_SERVER_ERROR, message)
            }
            Self::Process(ProcessError::NotInProcessMap(port)) => {
                let message = format!("Requested Port {} not in Process Dictionary", port);
                (StatusCode::NOT_FOUND, message)
            }
            Self::Process(ProcessError::Custom(message)) => (StatusCode::BAD_REQUEST, message),
            Self::Process(
                ProcessError::StartError(message) | ProcessError::TerminateError(message),
            ) => (StatusCode::BAD_REQUEST, message),
            Self::Map(MapError::NotFound(e)) => {
                tracing::debug!("Error: {}", e.to_string());
                (StatusCode::NOT_FOUND, e.to_string())
            }
            Self::Map(MapError::Other(e)) => {
                tracing::debug!("Error: {}", e.to_string());
                (StatusCode::BAD_REQUEST, e.to_string())
            }
            Self::Download(DownloadError::Io(e)) | Self::Download(DownloadError::TempFile(e)) => {
                tracing::debug!("Error: {}", e.to_string());
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
            Self::Download(DownloadError::ZipError(e)) => {
                tracing::debug!("Error: {}", e.to_string());
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
            Self::Download(DownloadError::FileNotFound(e)) => {
                tracing::debug!("Error: {}", e.to_string());
                (StatusCode::NOT_FOUND, e.to_string())
            }
            Self::Download(DownloadError::Unauthorized) => (
                StatusCode::UNAUTHORIZED,
                "IP and Port not in whitelist".to_string(),
            ),
            Self::Download(DownloadError::BotFolderNotFound(e)) => {
                tracing::debug!("Error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e)
            }
            Self::Download(DownloadError::Other(e)) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            Self::Download(DownloadError::NotAvailable(e)) => (StatusCode::NOT_IMPLEMENTED, e),
        };

        let body = Json(ApiErrorMessage {
            error: error_message,
        });

        (status, body).into_response()
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ApiErrorMessage {
    error: String,
}
