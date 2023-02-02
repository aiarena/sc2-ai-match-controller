#[cfg(feature = "swagger")]
mod docs;
mod routes;
mod utils;

#[cfg(feature = "swagger")]
use crate::docs::ApiDoc;
use crate::routes::{
    download_bot_data, download_bot_log, download_controller_log, start_bot, terminate_bot,
};
use axum::http::Request;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use axum::{error_handling::HandleErrorLayer, http::StatusCode};
use common::api::health;
use common::api::process::{
    shutdown, stats, stats_all, stats_host, status, terminate_all, ProcessMap,
};
use common::api::state::AppState;
use common::configuration::{get_config_from_proxy, get_host_url, get_proxy_url_from_env};
use common::logging::init_logging;
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{debug, Span};
#[cfg(feature = "swagger")]
use utoipa::OpenApi;
#[cfg(feature = "swagger")]
use utoipa_swagger_ui::SwaggerUi;

use std::path::Path;
use std::str::FromStr;
use std::{net::SocketAddr, time::Duration};

static PREFIX: &str = "ACBOT";

#[tokio::main]
async fn main() {
    let host_url = get_host_url(PREFIX, 8081);

    let proxy_url = get_proxy_url_from_env(PREFIX);
    let config_url = format!("http://{proxy_url}/configuration");
    let health_url = format!("http://{proxy_url}/health");

    let settings = get_config_from_proxy(config_url, health_url, PREFIX)
        .await
        .unwrap(); //panic if we can't get the config
    let env_log = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        format!(
            "info,common={},bot_controller={}",
            &settings.logging_level, &settings.logging_level
        )
    });
    let log_path = format!("{}/bot_controller", &settings.log_root);
    let log_file = "bot_controller.log";
    let full_path = Path::new(&log_path).join(log_file);
    if full_path.exists() {
        tokio::fs::remove_file(full_path).await.unwrap();
    }

    let (non_blocking_stdout, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let non_blocking_file = tracing_appender::rolling::never(&log_path, log_file);
    init_logging(&env_log, non_blocking_stdout, non_blocking_file);

    let process_map = ProcessMap::default();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    let state = AppState {
        process_map,
        settings,
        shutdown_sender: tx,
        extra_info: Default::default(),
    };

    #[allow(unused_mut)]
    let mut router = Router::<AppState>::new();
    #[cfg(feature = "swagger")]
    {
        router = router
            .merge(SwaggerUi::new("/swagger-ui/").url("/api-doc/openapi.json", ApiDoc::openapi()));
    }
    // Compose the routes
    let app = router
        .route("/start", post(start_bot))
        .route("/stats/:port", get(stats))
        .route("/status/:port", get(status))
        .route("/stats/host", get(stats_host))
        .route("/stats_all", get(stats_all))
        .route("/terminate_all", post(terminate_all))
        .route("/terminate/:bot_name", post(terminate_bot))
        .route("/shutdown", post(shutdown))
        .route("/download/controller_log", get(download_controller_log))
        .route("/download/bot/:port/log", get(download_bot_log))
        .route("/download/bot/:port/data", get(download_bot_data))
        // Add middleware to all routes
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &Request<_>, _span: &Span| {
                    tracing::debug!("started {} {}", request.method(), request.uri().path());
                })
                .on_response(|_response: &Response, latency: Duration, _span: &Span| {
                    tracing::debug!("response generated in {:?}", latency);
                }),
        )
        .route("/health", get(health))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        Ok(StatusCode::REQUEST_TIMEOUT)
                    } else {
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled internal error: {error}"),
                        ))
                    }
                }))
                .timeout(Duration::from_secs(120))
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(DefaultMakeSpan::new())
                        .on_request(DefaultOnRequest::new())
                        .on_response(DefaultOnResponse::new()),
                )
                .into_inner(),
        )
        .with_state(state.clone());

    let addr = SocketAddr::from_str(&host_url).unwrap();
    tracing::debug!("listening on {}", addr);
    let graceful_server = axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            tokio::select! {
                _ = rx.recv() => {},
                _ = shutdown_signal() => {},
            }
        });

    if let Err(e) = graceful_server.await {
        tracing::error!("server error: {}", e);
    }
}
/// Tokio signal handler that will wait for a user to press CTRL+C.
/// We use this in our hyper `Server` method `with_graceful_shutdown`.
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    debug!("signal received, starting graceful shutdown");
}
