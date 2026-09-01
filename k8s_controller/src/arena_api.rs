use anyhow::{anyhow, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct GraphQLResponse {
    data: Option<GetNextMatchData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetNextMatchData {
    get_next_match: Option<GetNextMatch>,
}

#[derive(Debug, Deserialize)]
struct GetNextMatch {
    #[serde(rename = "match")]
    match_info: Option<MatchInfo>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchInfo {
    pub id: String,
    pub participant1: Participant,
    pub participant2: Participant,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub name: String,
    pub game_display_id: String,
}

const GET_NEXT_MATCH_QUERY: &str = r#"
mutation {
  getNextMatch {
    match {
      id
      participant1 {
        name
        gameDisplayId
      }
      participant2 {
        name
        gameDisplayId
      }
    }
  }
}
"#;

pub async fn get_next_match(website_url: &str, token: &str) -> anyhow::Result<MatchInfo> {
    let client = Client::new();

    let body = serde_json::json!({
        "query": GET_NEXT_MATCH_QUERY,
    });

    let url = format!("{}/graphql/", website_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {}", token))
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .context("Failed to send GraphQL request")?;

    let text = resp.text().await.context("Failed to read response body")?;

    let parsed: GraphQLResponse = serde_json::from_str(&text).context("Failed to parse GraphQL response")?;

    let mut match_info = parsed
        .data
        .ok_or_else(|| anyhow!("GraphQL response has no data"))?
        .get_next_match
        .ok_or_else(|| anyhow!("GraphQL response has no getNextMatch"))?
        .match_info
        .ok_or_else(|| anyhow!("GraphQL response has no match"))?;

    match_info.id = decode_base64_id(&match_info.id)
        .map(|n| n.to_string())
        .unwrap_or_else(|| "0".to_string());

    Ok(match_info)
}

fn decode_base64_id(encoded: &str) -> Option<u32> {
    let bytes = STANDARD.decode(encoded).ok()?;
    let decoded = String::from_utf8(bytes).ok()?;
    let id_str = decoded.rsplit(':').next()?;
    id_str.parse().ok()
}
