use axum::body::Bytes;
use axum::body::StreamBody;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use reqwest::header::HeaderName;
use tokio_util::io::ReaderStream;
#[cfg(feature = "swagger")]
use utoipa::ToSchema;

pub mod errors;
pub mod process;
pub mod state;

#[tracing::instrument]
#[cfg_attr(feature = "swagger", utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Check health of controller")
    )
))]
pub async fn health() -> impl IntoResponse {
    (StatusCode::OK, "Ok")
}

pub type FileResponse = (
    [(HeaderName, &'static str); 1],
    StreamBody<ReaderStream<tokio::fs::File>>,
);

pub type BytesResponse = ([(HeaderName, &'static str); 1], Bytes);
