use common::api::api_reference::aiarena::aiarena_api_client::{
    AiArenaApiClient, CacheDownloadRequest,
};
use common::configuration::ac_config::ACConfig;
use common::models::aiarena::aiarena_match::AiArenaMatch;
use httpmock::prelude::*;
use httpmock::MockServer;
use serde_json::json;
use url::{ParseError, Url};

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
        when.method(GET).path("/media/maps/AutomatonLE");
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
        when.method(GET).path("/api/arenaclient/matches/1/1/zip/");
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
        when.method(GET).path("/api/arenaclient/matches/1/2/zip/");
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
        when.method(GET).path("/api/arenaclient/matches/1/1/data/");
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
        when.method(GET).path("/api/arenaclient/matches/1/2/data/");
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

    let bot1_request = CacheDownloadRequest {
        unique_key: format!("{}_zip", get_match_response.bot1.name),
        url: get_match_response.bot1.bot_zip,
        md5_hash: get_match_response.bot1.bot_zip_md5hash,
    };

    mockserver.mock(|when, then| {
        when.method(POST).path("/download").json_body(json!({
            "uniqueKey": bot1_request.unique_key,
            "url": bot1_request.url,
            "md5hash": bot1_request.md5_hash
        }));
        then.status(200).body(include_bytes!(
            "../../../testing/api-based/zip_files/basic_bot.zip"
        ));
    });
    let bot2_request = CacheDownloadRequest {
        unique_key: format!("{}_zip", get_match_response.bot2.name),
        url: get_match_response.bot2.bot_zip,
        md5_hash: get_match_response.bot2.bot_zip_md5hash,
    };
    mockserver.mock(|when, then| {
        when.method(POST).path("/download").json_body(json!({
            "uniqueKey": bot2_request.unique_key,
            "url": bot2_request.url,
            "md5hash": bot2_request.md5_hash
        }));
        then.status(200).body(include_bytes!(
            "../../../testing/api-based/zip_files/loser_bot.zip"
        ));
    });

    mockserver.mock(|when, then| {
        when.method(POST)
            .path("/upload")
            .query_param_exists("uniqueKey");
        then.status(200);
    });

    mockserver
}

fn change_url_host(original: &str, new_host: &str) -> Result<Url, ParseError> {
    let url = Url::parse(original)?;
    let host = Url::parse(new_host)?;
    host.join(url.path())
}
