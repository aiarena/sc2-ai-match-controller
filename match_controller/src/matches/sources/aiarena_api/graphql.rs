use anyhow::{anyhow, Context};
use base64::Engine;
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
                    // TODO: Implement incremental cooldown
                    // 10s, 20s, 40s, 80s, 120s, 120s, until 10m.
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
}

async fn upload_file(
    website_url: &str,
    token: &str,
    file_path: &Path,
) -> anyhow::Result<String> {
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

    info!("Uploading {} ({} KB) -> {}", file_path.display(), file_size_kb, upload_id);
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
                error!(
                    "Submit result attempt {}/{} failed: {}",
                    attempt, limit, e
                );
                last_err = Some(e);
                if attempt < limit {
                    // TODO: Implement incremental cooldown
                    // 10s, 20s, 40s, 80s, 120s, 120s, until 10m.
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    }
    Err(last_err.unwrap())
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

    // TODO: Remove this trace before old API is retired
    info!("Submitting result: {}", body);

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

    // TODO: Remove this trace before old API is retired
    info!("Response to SubmitResult: |{}|", text);

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

pub fn encode_match_id(text: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("MatchType:{}", text))
}
