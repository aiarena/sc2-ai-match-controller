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
    /// The extra command line the match requester asked each bot to be started
    /// with, exactly as they typed it. Empty for ladder matches, which never
    /// carry one. Nullable on the website, hence the Option.
    #[serde(default)]
    pub bot1_args: Option<String>,
    #[serde(default)]
    pub bot2_args: Option<String>,
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
      bot1Args
      bot2Args
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

    parse_next_match(&text)
}

fn parse_next_match(text: &str) -> anyhow::Result<MatchInfo> {
    let parsed: GraphQLResponse = serde_json::from_str(text).context("Failed to parse GraphQL response")?;

    let mut match_info = parsed
        .data
        .ok_or_else(|| anyhow!("GraphQL response has no data"))?
        .get_next_match
        .ok_or_else(|| anyhow!("GraphQL response has no getNextMatch"))?
        .match_info
        .ok_or_else(|| anyhow!("GraphQL response has no match"))?;

    match_info.id = decode_base64_id(&match_info.id).map(|n| n.to_string()).unwrap_or_else(|| "0".to_string());

    Ok(match_info)
}

fn decode_base64_id(encoded: &str) -> Option<u32> {
    let bytes = STANDARD.decode(encoded).ok()?;
    let decoded = String::from_utf8(bytes).ok()?;
    let id_str = decoded.rsplit(':').next()?;
    id_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_next_match;

    // Base64 of "MatchType:42", the global ID shape the website hands out.
    const MATCH_ID: &str = "TWF0Y2hUeXBlOjQy";

    fn response(match_fields: &str) -> String {
        format!(
            r#"{{"data": {{"getNextMatch": {{"match": {{
                "id": "{MATCH_ID}",
                "participant1": {{"name": "basic_bot", "gameDisplayId": "bot-id-1"}},
                "participant2": {{"name": "loser_bot", "gameDisplayId": "bot-id-2"}}
                {match_fields}
            }}}}}}}}"#
        )
    }

    #[test]
    fn reads_bot_args_from_the_response() {
        // Pins the field names against the website's schema: these are spelled
        // bot1Args/bot2Args there, and a silent mismatch here would look
        // exactly like a match that was requested without any arguments.
        let m = parse_next_match(&response(r#", "bot1Args": "--tournament=worldcup", "bot2Args": "--tournament=worldcup --build=\"all in\"""#)).unwrap();

        assert_eq!(m.id, "42");
        assert_eq!(m.bot1_args.as_deref(), Some("--tournament=worldcup"));
        assert_eq!(m.bot2_args.as_deref(), Some(r#"--tournament=worldcup --build="all in""#));
    }

    #[test]
    fn a_ladder_match_carries_no_bot_args() {
        let m = parse_next_match(&response(r#", "bot1Args": "", "bot2Args": """#)).unwrap();

        assert_eq!(m.bot1_args.as_deref(), Some(""));
        assert_eq!(m.bot2_args.as_deref(), Some(""));
    }

    #[test]
    fn null_or_missing_bot_args_is_not_an_error() {
        // The website's columns are nullable, so both shapes can arrive.
        let m = parse_next_match(&response(r#", "bot1Args": null, "bot2Args": null"#)).unwrap();
        assert_eq!(m.bot1_args, None);

        let m = parse_next_match(&response("")).unwrap();
        assert_eq!(m.participant1.name, "basic_bot");
        assert_eq!(m.bot1_args, None);
        assert_eq!(m.bot2_args, None);
    }
}
