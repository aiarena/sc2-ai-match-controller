use anyhow::{anyhow, Context};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;

use crate::graphql::post_graphql;
use crate::request::MatchRequest;
use crate::settings::Settings;

pub struct DownloadLinks {
    pub map: String,
    pub bot1_zip: String,
    pub bot1_data: Option<String>,
    pub bot2_zip: String,
    pub bot2_data: Option<String>,
}

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
        playsRace { databaseId }
        botZipUrl
        botDataUrl
      }
      participant2 {
        name
        gameDisplayId
        playsRace { databaseId }
        botZipUrl
        botDataUrl
      }
    }
  }
}
"#;

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
struct BotRaceInfo {
    database_id: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ParticipantInfo {
    name: String,
    game_display_id: String,
    plays_race: BotRaceInfo,
    bot_zip_url: String,
    bot_data_url: Option<String>,
}

pub async fn fetch_match_request(settings: &Settings) -> anyhow::Result<(MatchRequest, DownloadLinks)> {
    let body = serde_json::json!({ "query": GET_NEXT_MATCH_QUERY });
    let text = post_graphql(settings, "request", body).await?;
    let parsed: GetNextMatchResponse = serde_json::from_str(&text).context("Failed to parse getNextMatch response")?;

    let m = parsed
        .data
        .ok_or_else(|| anyhow!("getNextMatch response has no data"))?
        .get_next_match
        .ok_or_else(|| anyhow!("getNextMatch response has no getNextMatch"))?
        .match_info
        .ok_or_else(|| anyhow!("getNextMatch returned no match"))?;

    let match_id = STANDARD
        .decode(&m.id)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| s.rsplit(':').next().and_then(|id| id.parse().ok()))
        .unwrap_or(0);
    let match_request = MatchRequest {
        match_id,
        map_name: format!("{}.SC2Map", m.map.name),
        player_1_id: m.participant1.game_display_id.clone(),
        player_1_name: m.participant1.name.clone(),
        player_1_race: m.participant1.plays_race.database_id,
        player_2_id: m.participant2.game_display_id.clone(),
        player_2_name: m.participant2.name.clone(),
        player_2_race: m.participant2.plays_race.database_id,
    };
    let download_links = DownloadLinks {
        map: m.map.download_link,
        bot1_zip: m.participant1.bot_zip_url,
        bot1_data: m.participant1.bot_data_url,
        bot2_zip: m.participant2.bot_zip_url,
        bot2_data: m.participant2.bot_data_url,
    };
    Ok((match_request, download_links))
}
