use anyhow::{anyhow, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use common::models::aiarena::aiarena_bot::AiArenaBot;
use common::models::aiarena::aiarena_map::AiArenaMap;
use common::models::aiarena::aiarena_match::AiArenaMatch;
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
struct MatchInfo {
    id: String,
    participant1: Participant,
    participant2: Participant,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Participant {
    name: String,
    game_display_id: String,
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

pub async fn get_next_match(website_url: &str, token: &str) -> anyhow::Result<AiArenaMatch> {
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

    let parsed: GraphQLResponse =
        serde_json::from_str(&text).context("Failed to parse GraphQL response")?;

    let match_info = parsed
        .data
        .ok_or_else(|| anyhow!("GraphQL response has no data"))?
        .get_next_match
        .ok_or_else(|| anyhow!("GraphQL response has no getNextMatch"))?
        .match_info
        .ok_or_else(|| anyhow!("GraphQL response has no match"))?;

    Ok(convert_to_aiarena_match(match_info))
}

fn convert_to_aiarena_match(m: MatchInfo) -> AiArenaMatch {
    let id = decode_base64_id(&m.id).unwrap_or(0);
    AiArenaMatch {
        id,
        bot1: AiArenaBot {
            id: 0,
            name: m.participant1.name,
            game_display_id: m.participant1.game_display_id,
            bot_zip: String::new(),
            bot_zip_md5hash: String::new(),
            bot_data: None,
            bot_data_md5hash: None,
            plays_race: String::new(),
            _type: String::new(),
            bot_base: None,
        },
        bot2: AiArenaBot {
            id: 0,
            name: m.participant2.name,
            game_display_id: m.participant2.game_display_id,
            bot_zip: String::new(),
            bot_zip_md5hash: String::new(),
            bot_data: None,
            bot_data_md5hash: None,
            plays_race: String::new(),
            _type: String::new(),
            bot_base: None,
        },
        map: AiArenaMap {
            name: String::new(),
            file: String::new(),
            file_hash: None,
        },
        game_base: None,
    }
}

fn decode_base64_id(encoded: &str) -> Option<u32> {
    let bytes = STANDARD.decode(encoded).ok()?;
    let decoded = String::from_utf8(bytes).ok()?;
    let id_str = decoded.rsplit(':').next()?;
    id_str.parse().ok()
}
