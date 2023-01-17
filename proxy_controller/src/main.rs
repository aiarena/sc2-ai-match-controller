#![allow(dead_code)]
mod api_reference;
#[cfg(feature = "swagger")]
mod docs;
mod game;
mod match_scheduler;
pub mod matches;
mod routes;
mod state;
pub mod websocket;
mod ws_routes;

use crate::match_scheduler::match_scheduler;
use crate::matches::sources::aiarena_api::HttpApiSource;
use crate::matches::sources::test_source::TestSource;
use crate::matches::sources::{FileSource, MatchSource};
use crate::routes::{configuration, download_bot, download_bot_data, download_map};
use crate::state::ProxyState;
use crate::ws_routes::websocket_handler;
use axum::{http::Request, response::Response};
use common::api::health;
use common::axum::error_handling::HandleErrorLayer;
use common::axum::http::StatusCode;
use common::axum::routing::{get, post};
use common::axum::Router;
use common::config::{Config, FileFormat};
use common::configuration::ac_config::{ACConfig, RunType};
use common::configuration::get_host_url;
use common::logging::init_logging;
use common::parking_lot::RwLock;
use common::tower::ServiceBuilder;
use common::tower_http::trace::TraceLayer;
use common::tower_http::BoxError;
use common::{axum, config, tokio, tower, tracing, tracing_appender};
use std::net::SocketAddr;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tracing::Span;

static PREFIX: &str = "acproxy";

#[tokio::main]
async fn main() {
    let host_url = get_host_url(PREFIX, 8080);

    let settings = setup_proxy_config();
    let log_level = &settings.logging_level;
    let env_log = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| format!("info,proxy_controller={}", log_level));
    let log_path = format!("{}/proxy_controller", &settings.log_root);
    let log_file = "proxy_controller.log";
    let full_path = Path::new(&log_path).join(log_file);
    if full_path.exists() {
        common::tokio::fs::remove_file(full_path).await.unwrap();
    }
    let (non_blocking_stdout, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let non_blocking_file = tracing_appender::rolling::never(&log_path, log_file);
    init_logging(&env_log, non_blocking_stdout, non_blocking_file);

    let match_source: Box<dyn MatchSource> = match settings.run_type {
        RunType::Local => Box::new(FileSource::new(settings.clone())),
        RunType::AiArena => Box::new(HttpApiSource::new(settings.clone()).unwrap()),
        RunType::Test => Box::new(TestSource::new(settings.clone())),
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    let app_state = Arc::new(RwLock::new(ProxyState {
        settings,
        players: Vec::default(),
        current_match: None,
        game_config: None,
        sc2_urls: Vec::with_capacity(2),
        map: None,
        ready: false,
        port_config: None,
        game_result: None,
        auth_whitelist: indexmap::IndexSet::default(),
        shutdown_sender: tx,
    }));

    tokio::spawn(match_scheduler(app_state.clone(), match_source));

    // Compose the routes
    let app = Router::<Arc<RwLock<ProxyState>>>::new()
        .route("/configuration", get(configuration))
        .route("/sc2api", get(websocket_handler))
        .layer(
            TraceLayer::new_for_http()
                .on_request(|request: &Request<_>, _span: &Span| {
                    tracing::trace!("started {} {}", request.method(), request.uri().path());
                })
                .on_response(|_response: &Response, latency: Duration, _span: &Span| {
                    tracing::trace!("response generated in {:?}", latency);
                }),
        )
        .route("/health", get(health))
        .route("/download_bot", post(download_bot))
        .route("/download_map", get(download_map))
        .route("/download_bot_data", post(download_bot_data))
        // .route("/start", post(start_bot))
        // .route("/stats/:bot_name", get(stats))
        // .route("/stats/host", get(stats_host))
        // .route("/terminate/:bot_name", get(terminate_bot))
        // Add middleware to all routes
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
                .into_inner(),
        )
        .with_state(app_state);
    let addr = SocketAddr::from_str(&host_url).unwrap();

    tracing::debug!("listening on {}", addr);
    let graceful_server = axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async {
            let _ = rx.recv().await;
        });

    if let Err(e) = graceful_server.await {
        tracing::error!("server error: {}", e);
    }
}

fn setup_proxy_config() -> ACConfig {
    let default_config = include_str!("../../configs/default_config.toml");
    Config::builder()
        .add_source(config::File::from_str(default_config, FileFormat::Toml).required(true))
        .add_source(config::File::new("config.toml", FileFormat::Toml).required(false))
        .add_source(config::File::new("config.json", FileFormat::Json).required(false))
        .add_source(config::File::new("config.yaml", FileFormat::Yaml).required(false))
        .add_source(config::Environment::default().prefix(PREFIX))
        .build()
        .expect("Could not load config")
        .try_deserialize::<ACConfig>()
        .expect("Could not deserialize config")
}
