use anyhow::{anyhow, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use common::models::aiarena::aiarena_bot::AiArenaBot;
use common::models::aiarena::aiarena_map::AiArenaMap;
use common::models::aiarena::aiarena_match::AiArenaMatch;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::time::Duration;
use tracing::{error, info};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitResultInput {
    #[serde(rename = "match")]
    pub match_id: String,
    #[serde(rename = "type")]
    pub result_type: String,
    pub game_steps: u32,
    pub bot1_avg_step_time: f32,
    pub bot2_avg_step_time: f32,
    pub bot1_tags: Vec<String>,
    pub bot2_tags: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub replay_file: String,
    pub arenaclient_log: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bot1_data: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bot2_data: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bot1_log: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bot2_log: String,
}

// --- getNextMatch types ---

#[derive(Debug, Deserialize)]
struct GetNextMatchResponse {
    data: Option<GetNextMatchData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetNextMatchData {
    get_next_match: Option<GetNextMatchWrapper>,
}

#[derive(Debug, Deserialize)]
struct GetNextMatchWrapper {
    #[serde(rename = "match")]
    match_info: Option<MatchInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MatchInfo {
    id: String,
    map: MapInfo,
    participant1: ParticipantInfo,
    participant2: ParticipantInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MapInfo {
    name: String,
    download_link: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParticipantInfo {
    name: String,
    game_display_id: String,
    plays_race: String,
    #[serde(rename = "type")]
    bot_type: String,
    bot_zip_url: String,
    bot_data_url: Option<String>,
}

// --- requestUploadUrls / submitResult types ---

#[derive(Debug, Deserialize)]
struct UploadUrlsResponse {
    data: Option<RequestUploadUrlsData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestUploadUrlsData {
    request_upload_urls: Option<RequestUploadUrls>,
}

#[derive(Debug, Deserialize)]
struct RequestUploadUrls {
    uploads: Vec<UploadEntry>,
    errors: Vec<GraphQLFieldError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadEntry {
    upload: UploadInfo,
    upload_url: String,
}

#[derive(Debug, Deserialize)]
struct UploadInfo {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GraphQLFieldError {
    field: String,
    messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SubmitResultResponse {
    data: Option<SubmitResultData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmitResultData {
    submit_result: Option<SubmitResult>,
}

#[derive(Debug, Deserialize)]
struct SubmitResult {
    result: Option<ResultInfo>,
    errors: Vec<GraphQLFieldError>,
}

#[derive(Debug, Deserialize)]
struct ResultInfo {
    id: String,
}

// --- Queries ---

const GET_NEXT_MATCH_QUERY: &str = r#"
mutation {
  getNextMatch {
    match {
      id
      map {
        name
        downloadLink
      }
      participant1 {
        name
        gameDisplayId
        playsRace
        type
        botZipUrl
        botDataUrl
      }
      participant2 {
        name
        gameDisplayId
        playsRace
        type
        botZipUrl
        botDataUrl
      }
    }
  }
}
"#;

const REQUEST_UPLOAD_URLS_QUERY: &str = r#"
mutation($input: RequestUploadUrlsInput!) {
  requestUploadUrls(input: $input) {
    uploads {
      upload {
        id
      }
      uploadUrl
    }
    errors {
      field
      messages
    }
  }
}
"#;

const SUBMIT_RESULT_QUERY: &str = r#"
mutation($input: SubmitResultInput!) {
  submitResult(input: $input) {
    result {
      id
    }
    errors {
      field
      messages
    }
  }
}
"#;

// --- Public API ---

pub async fn get_next_match(website_url: &str, token: &str) -> anyhow::Result<AiArenaMatch> {
    let client = Client::new();
    let graphql_url = format!("{}/graphql/", website_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "query": GET_NEXT_MATCH_QUERY,
    });

    let resp = client
        .post(&graphql_url)
        .header("Authorization", format!("Token {}", token))
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to send getNextMatch GraphQL request")?;

    let text = resp
        .text()
        .await
        .context("Failed to read getNextMatch response body")?;

    let parsed: GetNextMatchResponse =
        serde_json::from_str(&text).context("Failed to parse getNextMatch response")?;

    let match_info = parsed
        .data
        .ok_or_else(|| anyhow!("getNextMatch response has no data"))?
        .get_next_match
        .ok_or_else(|| anyhow!("getNextMatch response has no getNextMatch"))?
        .match_info
        .ok_or_else(|| anyhow!("getNextMatch returned no match"))?;

    Ok(convert_match(match_info))
}

pub async fn upload_file_with_retries(
    website_url: &str,
    token: &str,
    file_path: &Path,
    retries: u32,
) -> anyhow::Result<String> {
    let limit = retries.max(1);
    let mut last_err = None;

    for attempt in 1..=limit {
        match upload_file(website_url, token, file_path).await {
            Ok(id) => return Ok(id),
            Err(e) => {
                error!(
                    "Upload attempt {}/{} failed for {}: {}",
                    attempt,
                    limit,
                    file_path.display(),
                    e
                );
                last_err = Some(e);
                if attempt < limit {
                    tokio::time::sleep(cooldown_duration(attempt)).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
}

pub async fn submit_result_with_retries(
    website_url: &str,
    token: &str,
    input: &SubmitResultInput,
    retries: u32,
) -> anyhow::Result<String> {
    let limit = retries.max(1);
    let mut last_err = None;

    for attempt in 1..=limit {
        match submit_result(website_url, token, input).await {
            Ok(id) => return Ok(id),
            Err(e) => {
                error!("Submit result attempt {}/{} failed: {}", attempt, limit, e);
                last_err = Some(e);
                if attempt < limit {
                    tokio::time::sleep(cooldown_duration(attempt)).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
}

pub fn encode_match_id(text: &str) -> String {
    STANDARD.encode(format!("MatchType:{}", text))
}

// --- Private helpers ---

fn cooldown_duration(attempt: u32) -> Duration {
    // 10s, 20s, 40s, 80s, then capped at 120s
    let secs = (10u64 * 2u64.pow(attempt - 1)).min(120);
    Duration::from_secs(secs)
}

fn decode_base64_id(encoded: &str) -> Option<u32> {
    let bytes = STANDARD.decode(encoded).ok()?;
    let decoded = String::from_utf8(bytes).ok()?;
    let id_str = decoded.rsplit(':').next()?;
    id_str.parse().ok()
}

fn convert_match(m: MatchInfo) -> AiArenaMatch {
    let id = decode_base64_id(&m.id).unwrap_or(0);
    AiArenaMatch {
        id,
        bot1: convert_bot(m.participant1),
        bot2: convert_bot(m.participant2),
        map: AiArenaMap {
            name: m.map.name,
            download_link: m.map.download_link,
        },
        game_base: None,
    }
}

fn convert_bot(p: ParticipantInfo) -> AiArenaBot {
    AiArenaBot {
        name: p.name,
        game_display_id: p.game_display_id,
        plays_race: p.plays_race,
        _type: p.bot_type,
        bot_zip_url: p.bot_zip_url,
        bot_data_url: p.bot_data_url,
        bot_base: None,
    }
}

async fn upload_file(website_url: &str, token: &str, file_path: &Path) -> anyhow::Result<String> {
    let client = Client::new();
    let graphql_url = format!("{}/graphql/", website_url.trim_end_matches('/'));

    // Step 1: Request a signed upload URL
    let body = serde_json::json!({
        "query": REQUEST_UPLOAD_URLS_QUERY,
        "variables": {
            "input": {
                "count": 1
            }
        }
    });

    let resp = client
        .post(&graphql_url)
        .header("Authorization", format!("Token {}", token))
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to send requestUploadUrls GraphQL request")?;

    let text = resp
        .text()
        .await
        .context("Failed to read requestUploadUrls response body")?;

    let parsed: UploadUrlsResponse =
        serde_json::from_str(&text).context("Failed to parse requestUploadUrls response")?;

    let upload_urls = parsed
        .data
        .ok_or_else(|| anyhow!("requestUploadUrls response has no data"))?
        .request_upload_urls
        .ok_or_else(|| anyhow!("requestUploadUrls response has no requestUploadUrls"))?;

    if !upload_urls.errors.is_empty() {
        let msgs: Vec<String> = upload_urls
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.field, e.messages.join(", ")))
            .collect();
        return Err(anyhow!("requestUploadUrls errors: {}", msgs.join("; ")));
    }

    let entry = upload_urls
        .uploads
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("requestUploadUrls returned no uploads"))?;

    let upload_id = entry.upload.id;
    let upload_url = entry.upload_url;

    // Step 2: Upload the file to the signed S3 URL
    let file_bytes = tokio::fs::read(file_path)
        .await
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;
    let file_size_kb = file_bytes.len() / 1024;

    info!(
        "Uploading {} ({} KB) -> {}",
        file_path.display(),
        file_size_kb,
        upload_id
    );
    client
        .put(&upload_url)
        .body(file_bytes)
        .send()
        .await
        .context("Failed to upload file to S3")?
        .error_for_status()
        .context("S3 upload returned an error status")?;

    // Step 3: Return the upload id
    Ok(upload_id)
}

async fn submit_result(
    website_url: &str,
    token: &str,
    input: &SubmitResultInput,
) -> anyhow::Result<String> {
    let client = Client::new();
    let graphql_url = format!("{}/graphql/", website_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "query": SUBMIT_RESULT_QUERY,
        "variables": {
            "input": input
        }
    });

    let resp = client
        .post(&graphql_url)
        .header("Authorization", format!("Token {}", token))
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to send submitResult GraphQL request")?;

    let text = resp
        .text()
        .await
        .context("Failed to read submitResult response body")?;

    let parsed: SubmitResultResponse =
        serde_json::from_str(&text).context("Failed to parse submitResult response")?;

    let submit_result = parsed
        .data
        .ok_or_else(|| anyhow!("submitResult response has no data"))?
        .submit_result
        .ok_or_else(|| anyhow!("submitResult response has no submitResult"))?;

    if !submit_result.errors.is_empty() {
        let msgs: Vec<String> = submit_result
            .errors
            .iter()
            .map(|e| format!("{}: {}", e.field, e.messages.join(", ")))
            .collect();
        return Err(anyhow!("submitResult errors: {}", msgs.join("; ")));
    }

    let result_id = submit_result
        .result
        .ok_or_else(|| anyhow!("submitResult returned no result"))?
        .id;

    Ok(result_id)
}
