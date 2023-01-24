mod arenaclient;
// mod old;
mod k8s_config;
mod k8s_processor;
mod state;
// #[cfg(feature = "swagger")]
// mod docs;

// #[cfg(feature = "swagger")]
// use crate::docs::ApiDoc;
use crate::k8s_processor::process;
use axum::http::Request;
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use axum::{error_handling::HandleErrorLayer, http::StatusCode};
use common::api::health;
use common::configuration::get_host_url;
use common::logging::init_logging;
use config::{Config, FileFormat};
use parking_lot::RwLock;
use state::AppState;
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{debug, info, Span};
#[cfg(feature = "swagger")]
use utoipa::OpenApi;
#[cfg(feature = "swagger")]
use utoipa_swagger_ui::SwaggerUi;

use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::{net::SocketAddr, time::Duration};
static VERSION: &str = env!("CARGO_PKG_VERSION");
static PREFIX: &str = "ACK8S";

#[tokio::main]
async fn main() {
    let host_url = get_host_url(PREFIX, 8085);

    let env_log =
        std::env::var("RUST_LOG").unwrap_or_else(|_| format!("info,k8s_controller={}", "debug"));
    let log_path = "/logs/k8s_controller".to_string();
    let log_file = "k8s_controller.log";
    let full_path = Path::new(&log_path).join(log_file);
    if full_path.exists() {
        tokio::fs::remove_file(full_path).await.unwrap();
    }
    let mut settings = setup_k8s_config();
    settings.version = Some(format!("v{}", VERSION));

    let (non_blocking_stdout, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let non_blocking_file = tracing_appender::rolling::never(&log_path, log_file);
    init_logging(&env_log, non_blocking_stdout, non_blocking_file);

    info!("Running version: {:?}", VERSION);

    let state = AppState {};
    let app_state = Arc::new(RwLock::new(state));

    #[allow(unused_mut)]
    let mut router = Router::<Arc<RwLock<AppState>>>::new();
    #[cfg(feature = "swagger")]
    {
        router = router
            .merge(SwaggerUi::new("/swagger-ui/").url("/api-doc/openapi.json", ApiDoc::openapi()));
    }

    tokio::spawn(process(settings));

    // Compose the routes
    let app = router
        // .route("/stats/:port", get(stats))
        // .route("/status/:port", get(status))
        // .route("/stats/host", get(stats_host))
        // .route("/stats_all", get(stats_all))
        // .route("/shutdown", post(shutdown))
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
                            format!("Unhandled internal error: {}", error),
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
        .with_state(app_state.clone());

    let addr = SocketAddr::from_str(&host_url).unwrap();
    tracing::debug!("listening on {}", addr);

    let graceful_server = axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            shutdown_signal().await;
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

fn setup_k8s_config() -> k8s_config::K8sConfig {
    let default_config = include_str!("../configs/default_config.toml");
    Config::builder()
        .add_source(config::File::from_str(default_config, FileFormat::Toml).required(true))
        .add_source(config::File::new("config.toml", FileFormat::Toml).required(false))
        .add_source(config::File::new("config.json", FileFormat::Json).required(false))
        .add_source(config::File::new("config.yaml", FileFormat::Yaml).required(false))
        .add_source(config::Environment::default().prefix(PREFIX))
        .build()
        .expect("Could not load config")
        .try_deserialize::<k8s_config::K8sConfig>()
        .expect("Could not deserialize config")
}
