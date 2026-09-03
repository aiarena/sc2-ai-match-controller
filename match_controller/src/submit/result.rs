use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};

use crate::graphql::{post_graphql, GraphQLFieldError};
use crate::settings::Settings;

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

pub async fn submit_result(settings: &Settings, input: &SubmitResultInput) -> anyhow::Result<String> {
    let body = serde_json::json!({
        "query": SUBMIT_RESULT_QUERY,
        "variables": { "input": input }
    });
    let text = post_graphql(settings, "submit", body).await?;
    let parsed: SubmitResultResponse = serde_json::from_str(&text).context("Failed to parse submitResult response")?;
    let submit_result = parsed
        .data
        .ok_or_else(|| anyhow!("submitResult response has no data"))?
        .submit_result
        .ok_or_else(|| anyhow!("submitResult response has no submitResult"))?;
    if !submit_result.errors.is_empty() {
        let msgs: Vec<String> = submit_result.errors.iter().map(|e| format!("{}: {}", e.field, e.messages.join(", "))).collect();
        return Err(anyhow!("submitResult errors: {}", msgs.join("; ")));
    }
    submit_result.result.ok_or_else(|| anyhow!("submitResult returned no result")).map(|r| r.id)
}
