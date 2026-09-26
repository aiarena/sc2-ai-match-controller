use axum::{
    body::Body,
    extract::{Json, Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Router,
};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const ETAG_BOT1_ZIP: &str = "\"test-etag-basic-bot-zip\"";
const ETAG_BOT1_DATA: &str = "\"test-etag-basic-bot-data\"";
const ETAG_BOT2_ZIP: &str = "\"test-etag-loser-bot-zip\"";
const ETAG_BOT2_DATA: &str = "\"test-etag-loser-bot-data\"";
const ETAG_MAP: &str = "\"test-etag-automaton-map\"";

#[derive(Clone)]
struct AppState {
    // Counts how many node match queries have been made.
    // Match 1 (count == 1): cold cache — cache GET returns 404, source GETs serve full files.
    // Match 2 (count >= 2): warm cache — cache GET serves files, source GETs return ETag only (no body).
    match_count: Arc<AtomicUsize>,
}

#[derive(Debug, Deserialize)]
struct GraphQLBody {
    query: String,
    variables: Option<serde_json::Value>,
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

    let state = AppState {
        match_count: Arc::new(AtomicUsize::new(0)),
    };

    let protected_routes = Router::new()
        .route("/graphql/", post(graphql_handler))
        .layer(middleware::from_fn(check_authorization));

    let public_routes = Router::new()
        .route(
            "/api/arenaclient/matches/1/1/zip/",
            get(get_bot1_zip).head(head_bot1_zip),
        )
        .route(
            "/api/arenaclient/matches/1/1/data/",
            get(get_bot1_data).head(head_bot1_data),
        )
        .route(
            "/api/arenaclient/matches/1/2/zip/",
            get(get_bot2_zip).head(head_bot2_zip),
        )
        .route(
            "/api/arenaclient/matches/1/2/data/",
            get(get_bot2_data).head(head_bot2_data),
        )
        .route("/media/maps/AutomatonLE", get(get_map).head(head_map))
        .route("/s3-upload/{id}", put(s3_upload))
        .route("/cache/{*key}", get(cache_download).put(cache_upload));

    let app = Router::new()
        .merge(protected_routes)
        .merge(public_routes)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

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

async fn graphql_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<GraphQLBody>,
) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let query = &body.query;
    if query.contains("node") {
        let match_id = body.variables
            .as_ref()
            .and_then(|v| v["id"].as_str())
            .unwrap_or("TWF0Y2hUeXBlOjE=");
        let count = state.match_count.fetch_add(1, Ordering::SeqCst) + 1;
        tracing::info!("node match query #{} for id {}", count, match_id);
        graphql_get_match(host, match_id)
    } else if query.contains("requestUploadUrls") {
        graphql_request_upload_urls(host)
    } else if query.contains("submitResult") {
        graphql_submit_result()
    } else {
        (StatusCode::BAD_REQUEST, "Unknown mutation").into_response()
    }
}

fn graphql_get_match(host: &str, match_id: &str) -> Response {
    let base_url = format!("http://{}", host);
    let json_str = include_str!("../data/match.json");
    let modified = json_str.replace("https://aiarena.net", &base_url);
    let m: serde_json::Value = serde_json::from_str(&modified).unwrap();

    let database_id: serde_json::Value = match_id.parse::<u64>()
        .map(serde_json::Value::from)
        .unwrap_or_else(|_| m["id"].clone());

    let response = json!({
        "data": {
            "node": {
                "id": match_id,
                "databaseId": database_id,
                "map": {
                    "name": m["map"]["name"],
                    "downloadLink": m["map"]["download_link"]
                },
                "participant1": {
                    "name": m["bot1"]["name"],
                    "gameDisplayId": m["bot1"]["game_display_id"],
                    "playsRace": m["bot1"]["plays_race"],
                    "type": m["bot1"]["type"],
                    "botZipUrl": m["bot1"]["bot_zip_url"],
                    "botDataUrl": m["bot1"]["bot_data_url"]
                },
                "participant2": {
                    "name": m["bot2"]["name"],
                    "gameDisplayId": m["bot2"]["game_display_id"],
                    "playsRace": m["bot2"]["plays_race"],
                    "type": m["bot2"]["type"],
                    "botZipUrl": m["bot2"]["bot_zip_url"],
                    "botDataUrl": m["bot2"]["bot_data_url"]
                }
            }
        }
    });

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&response).unwrap(),
    )
        .into_response()
}

fn graphql_request_upload_urls(host: &str) -> Response {
    let base_url = format!("http://{}", host);
    let response = json!({
        "data": {
            "requestUploadUrls": {
                "uploads": [
                    {
                        "upload": {"id": "test-upload-1"},
                        "uploadUrl": format!("{}/s3-upload/test-upload-1", base_url)
                    }
                ],
                "errors": []
            }
        }
    });
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&response).unwrap(),
    )
        .into_response()
}

fn graphql_submit_result() -> Response {
    let response = json!({
        "data": {
            "submitResult": {
                "result": {"id": "test-result-1"},
                "errors": []
            }
        }
    });
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        serde_json::to_string(&response).unwrap(),
    )
        .into_response()
}

// Source URL handlers — return 500 in match 2 to verify no direct downloads happen.

async fn get_map(State(state): State<AppState>) -> Response {
    if state.match_count.load(Ordering::SeqCst) >= 2 {
        return (StatusCode::OK, [(header::ETAG, ETAG_MAP)]).into_response();
    }
    let map_data = include_bytes!("../../testing-maps/AutomatonLE.SC2Map");
    (
        StatusCode::OK,
        [(header::ETAG, ETAG_MAP)],
        Body::from(&map_data[..]),
    )
        .into_response()
}

async fn head_map() -> Response {
    (StatusCode::OK, [(header::ETAG, ETAG_MAP)]).into_response()
}

async fn get_bot1_zip(State(state): State<AppState>) -> Response {
    if state.match_count.load(Ordering::SeqCst) >= 2 {
        return (StatusCode::OK, [(header::ETAG, ETAG_BOT1_ZIP)]).into_response();
    }
    let bot_data = include_bytes!("../data/basic_bot.zip");
    (
        StatusCode::OK,
        [(header::ETAG, ETAG_BOT1_ZIP)],
        Body::from(&bot_data[..]),
    )
        .into_response()
}

async fn head_bot1_zip() -> Response {
    (StatusCode::OK, [(header::ETAG, ETAG_BOT1_ZIP)]).into_response()
}

async fn get_bot1_data(State(state): State<AppState>) -> Response {
    if state.match_count.load(Ordering::SeqCst) >= 2 {
        return (StatusCode::OK, [(header::ETAG, ETAG_BOT1_DATA)]).into_response();
    }
    let bot_data = include_bytes!("../data/basic_bot_data.zip");
    (
        StatusCode::OK,
        [(header::ETAG, ETAG_BOT1_DATA)],
        Body::from(&bot_data[..]),
    )
        .into_response()
}

async fn head_bot1_data() -> Response {
    (StatusCode::OK, [(header::ETAG, ETAG_BOT1_DATA)]).into_response()
}

async fn get_bot2_zip(State(state): State<AppState>) -> Response {
    if state.match_count.load(Ordering::SeqCst) >= 2 {
        return (StatusCode::OK, [(header::ETAG, ETAG_BOT2_ZIP)]).into_response();
    }
    let bot_data = include_bytes!("../data/loser_bot.zip");
    (
        StatusCode::OK,
        [(header::ETAG, ETAG_BOT2_ZIP)],
        Body::from(&bot_data[..]),
    )
        .into_response()
}

async fn head_bot2_zip() -> Response {
    (StatusCode::OK, [(header::ETAG, ETAG_BOT2_ZIP)]).into_response()
}

async fn get_bot2_data(State(state): State<AppState>) -> Response {
    if state.match_count.load(Ordering::SeqCst) >= 2 {
        return (StatusCode::OK, [(header::ETAG, ETAG_BOT2_DATA)]).into_response();
    }
    let bot_data = include_bytes!("../data/loser_bot_data.zip");
    (
        StatusCode::OK,
        [(header::ETAG, ETAG_BOT2_DATA)],
        Body::from(&bot_data[..]),
    )
        .into_response()
}

async fn head_bot2_data() -> Response {
    (StatusCode::OK, [(header::ETAG, ETAG_BOT2_DATA)]).into_response()
}

async fn s3_upload(Path(id): Path<String>) -> Response {
    tracing::debug!("S3 upload for id: {}", id);
    StatusCode::OK.into_response()
}

async fn cache_download(State(state): State<AppState>, Path(key): Path<String>) -> Response {
    let count = state.match_count.load(Ordering::SeqCst);
    tracing::debug!("Cache download (match {}) for key: {}", count, key);

    // Match 1: cold cache — always miss so the client falls back to source URLs.
    if count <= 1 {
        tracing::debug!("Cache miss (match 1) for key: {}", key);
        return StatusCode::NOT_FOUND.into_response();
    }

    let Some((name, etag)) = key.rsplit_once('/') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let etag = format!("\"{}\"", etag);
    let etag = etag.as_str();

    // Match 2+: warm cache — serve when name and etag match.
    let served: Option<&'static [u8]> = match name {
        "bot-code/basic_bot" if etag == ETAG_BOT1_ZIP => {
            Some(&include_bytes!("../data/basic_bot.zip")[..])
        }
        "bot-data/basic_bot" if etag == ETAG_BOT1_DATA => {
            Some(&include_bytes!("../data/basic_bot_data.zip")[..])
        }
        "bot-code/loser_bot" if etag == ETAG_BOT2_ZIP => {
            Some(&include_bytes!("../data/loser_bot.zip")[..])
        }
        "bot-data/loser_bot" if etag == ETAG_BOT2_DATA => {
            Some(&include_bytes!("../data/loser_bot_data.zip")[..])
        }
        "map/AutomatonLE.SC2Map" if etag == ETAG_MAP => {
            Some(&include_bytes!("../../testing-maps/AutomatonLE.SC2Map")[..])
        }
        _ => None,
    };

    match served {
        Some(bytes) => (StatusCode::OK, Body::from(bytes)).into_response(),
        None => {
            tracing::error!(
                "Cache miss in match {} for key '{}' — name or etag mismatch",
                count,
                key
            );
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

async fn cache_upload(Path(key): Path<String>) -> Response {
    tracing::debug!("Cache upload for key: {}", key);
    StatusCode::OK.into_response()
}
