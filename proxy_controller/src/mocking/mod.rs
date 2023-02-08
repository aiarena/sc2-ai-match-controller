use common::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
use common::configuration::ac_config::ACConfig;
use httpmock::prelude::*;
use httpmock::MockServer;
use url::{ParseError, Url};
use common::models::aiarena::aiarena_match::AiArenaMatch;

pub fn setup_mock_server(settings: &ACConfig) -> MockServer {
    let mockserver = MockServer::start();
    let mock_url = mockserver.base_url();
    let token = settings.api_token.clone().unwrap();

    let mut get_match_response: AiArenaMatch = serde_json::from_str(include_str!(
        "../../../testing/api-based/responses/get_match_response.json"
    ))
    .unwrap();

    get_match_response.map.file = change_url_host(&get_match_response.map.file, &mock_url)
        .unwrap()
        .to_string();
    get_match_response.bot1.bot_zip = change_url_host(&get_match_response.bot1.bot_zip, &mock_url)
        .unwrap()
        .to_string();
    get_match_response.bot2.bot_zip = change_url_host(&get_match_response.bot2.bot_zip, &mock_url)
        .unwrap()
        .to_string();
    if get_match_response.bot1.bot_data.is_some() {
        get_match_response.bot1.bot_data = Some(
            change_url_host(&get_match_response.bot1.bot_data.unwrap(), &mock_url)
                .unwrap()
                .to_string(),
        );
    }
    if get_match_response.bot2.bot_data.is_some() {
        get_match_response.bot2.bot_data = Some(
            change_url_host(&get_match_response.bot2.bot_data.unwrap(), &mock_url)
                .unwrap()
                .to_string(),
        );
    }

    mockserver.mock(|when, then| {
        when.method(POST)
            .path(AiArenaApiClient::API_MATCHES_ENDPOINT)
            .header("Authorization", format!("Token {token}",));
        then.status(200)
            .json_body_obj(&get_match_response)
            .header("Content-Type", "application/json");
    });
    mockserver.mock(|when, then| {
        when.method(GET)
            .path("/media/maps/AutomatonLE")
            .header("authorization", format!("Token {token}",));
        then.status(200).body(include_bytes!(
            "../../../testing/testing-maps/AutomatonLE.SC2Map"
        ));
    });
    mockserver.mock(|when, then| {
        when.method(GET)
            .path("/api/arenaclient/matches/1/1/zip/")
            .header("Authorization", format!("Token {token}",));
        then.status(200).body(include_bytes!(
            "../../../testing/api-based/zip_files/basic_bot.zip"
        ));
    });

    mockserver.mock(|when, then| {
        when.method(GET)
            .path("/api/arenaclient/matches/1/2/zip/")
            .header("Authorization", format!("Token {token}",));
        then.status(200).body(include_bytes!(
            "../../../testing/api-based/zip_files/loser_bot.zip"
        ));
    });
    mockserver.mock(|when, then| {
        when.method(GET)
            .path("/api/arenaclient/matches/1/1/data/")
            .header("Authorization", format!("Token {token}",));
        then.status(200).body(include_bytes!(
            "../../../testing/api-based/zip_files/basic_bot_data.zip"
        ));
    });
    mockserver.mock(|when, then| {
        when.method(GET)
            .path("/api/arenaclient/matches/1/2/data/")
            .header("Authorization", format!("Token {token}",));
        then.status(200).body(include_bytes!(
            "../../../testing/api-based/zip_files/loser_bot_data.zip"
        ));
    });

    mockserver.mock(|when, then| {
        when.method(POST)
            .path(AiArenaApiClient::API_RESULTS_ENDPOINT)
            .header("Authorization", format!("Token {token}",));
        then.status(200);
    });

    mockserver
}

fn change_url_host(original: &str, new_host: &str) -> Result<Url, ParseError> {
    let url = Url::parse(original)?;
    let host = Url::parse(new_host)?;
    host.join(url.path())
}
