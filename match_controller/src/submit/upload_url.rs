use anyhow::{anyhow, Context};
use serde::Deserialize;

use crate::graphql::{post_graphql, GraphQLFieldError};
use crate::settings::Settings;

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

pub async fn request_upload_url(settings: &Settings) -> anyhow::Result<(String, String)> {
    let body = serde_json::json!({
        "query": REQUEST_UPLOAD_URLS_QUERY,
        "variables": { "input": { "count": 1 } }
    });
    let text = post_graphql(settings, "urls", body).await?;
    let parsed: UploadUrlsResponse = serde_json::from_str(&text).context("Failed to parse requestUploadUrls response")?;

    let upload_urls = parsed
        .data
        .ok_or_else(|| anyhow!("requestUploadUrls response has no data"))?
        .request_upload_urls
        .ok_or_else(|| anyhow!("requestUploadUrls response has no requestUploadUrls"))?;

    if !upload_urls.errors.is_empty() {
        let msgs: Vec<String> = upload_urls.errors.iter().map(|e| format!("{}: {}", e.field, e.messages.join(", "))).collect();
        return Err(anyhow!("requestUploadUrls errors: {}", msgs.join("; ")));
    }

    let entry = upload_urls.uploads.into_iter().next().ok_or_else(|| anyhow!("requestUploadUrls returned no uploads"))?;

    Ok((entry.upload.id, entry.upload_url))
}
