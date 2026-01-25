use axum::{
    body::Body,
    extract::{Host, Json, Query, Request},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Deserialize, Serialize)]
struct DownloadRequest {
    #[serde(rename = "uniqueKey")]
    unique_key: String,
    url: String,
    #[serde(rename = "md5hash")]
    md5_hash: String,
}

#[derive(Debug, Deserialize)]
struct UploadParams {
    #[serde(rename = "uniqueKey")]
    unique_key: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "test_api_server=debug,tower_http=debug,axum=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let protected_routes = Router::new()
        .route("/api/arenaclient/v2/next-match/", post(next_match))
        .route("/api/arenaclient/v2/submit-result/", post(submit_result))
        .layer(middleware::from_fn(check_authorization));

    let public_routes = Router::new()
        .route("/api/arenaclient/matches/1/1/zip/", get(get_bot1_zip))
        .route("/api/arenaclient/matches/1/1/data/", get(get_bot1_data))
        .route("/api/arenaclient/matches/1/2/zip/", get(get_bot2_zip))
        .route("/api/arenaclient/matches/1/2/data/", get(get_bot2_data))
        .route("/download", post(download))
        .route("/media/maps/AutomatonLE", get(get_map))
        .route("/upload", post(upload));

    let app = Router::new()
        .merge(protected_routes)
        .merge(public_routes)
        .layer(TraceLayer::new_for_http());

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("Test API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn check_authorization(request: Request, next: Next) -> Response {
    let auth = request.headers().get("Authorization");
    if auth.is_none() || auth.unwrap().is_empty() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

async fn next_match(Host(host): Host) -> Response {
    let match_response = load_match_response_with_host(&host);

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&match_response).unwrap(),
    )
        .into_response()
}

async fn get_map() -> Response {
    let map_data = include_bytes!("../../testing-maps/AutomatonLE.SC2Map");
    (StatusCode::OK, Body::from(&map_data[..])).into_response()
}

async fn get_bot1_zip() -> Response {
    let bot_data = include_bytes!("../data/basic_bot.zip");
    (StatusCode::OK, Body::from(&bot_data[..])).into_response()
}

async fn get_bot2_zip() -> Response {
    let bot_data = include_bytes!("../data/loser_bot.zip");
    (StatusCode::OK, Body::from(&bot_data[..])).into_response()
}

async fn get_bot1_data() -> Response {
    let bot_data = include_bytes!("../data/basic_bot_data.zip");
    (StatusCode::OK, Body::from(&bot_data[..])).into_response()
}

async fn get_bot2_data() -> Response {
    let bot_data = include_bytes!("../data/loser_bot_data.zip");
    (StatusCode::OK, Body::from(&bot_data[..])).into_response()
}

async fn submit_result() -> Response {
    StatusCode::OK.into_response()
}

async fn download(Host(host): Host, Json(payload): Json<DownloadRequest>) -> Response {
    tracing::debug!("Download request: {:?}", payload);

    let match_response = load_match_response_with_host(&host);

    if payload.unique_key == "basic_bot_zip" {
        let bot1_md5 = match_response["bot1"]["bot_zip_md5hash"].as_str().unwrap();
        let bot1_url = match_response["bot1"]["bot_zip"].as_str().unwrap();

        if payload.url == bot1_url && payload.md5_hash == bot1_md5 {
            let bot_data = include_bytes!("../data/basic_bot.zip");
            return (StatusCode::OK, Body::from(&bot_data[..])).into_response();
        }
    }

    if payload.unique_key == "loser_bot_zip" {
        let bot2_md5 = match_response["bot2"]["bot_zip_md5hash"].as_str().unwrap();
        let bot2_url = match_response["bot2"]["bot_zip"].as_str().unwrap();

        if payload.url == bot2_url && payload.md5_hash == bot2_md5 {
            let bot_data = include_bytes!("../data/loser_bot.zip");
            return (StatusCode::OK, Body::from(&bot_data[..])).into_response();
        }
    }

    if payload.unique_key == "AutomatonLE" {
        let map_md5 = match_response["map"]["file_hash"].as_str().unwrap();
        let map_url = match_response["map"]["file"].as_str().unwrap();

        if payload.url == map_url && payload.md5_hash == map_md5 {
            let map_data = include_bytes!("../../testing-maps/AutomatonLE.SC2Map");
            return (StatusCode::OK, Body::from(&map_data[..])).into_response();
        }
    }

    StatusCode::NOT_FOUND.into_response()
}

fn load_match_response_with_host(host: &str) -> serde_json::Value {
    let base_url = format!("http://{}", host);
    
    let json_str = include_str!("../data/match.json");
    let modified_json = json_str.replace("https://aiarena.net", &base_url);
    serde_json::from_str(&modified_json).unwrap()
}

async fn upload(Query(params): Query<UploadParams>) -> Response {
    tracing::debug!("Upload request with uniqueKey: {}", params.unique_key);
    StatusCode::OK.into_response()
}
