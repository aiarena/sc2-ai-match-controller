use std::time::Duration;

use crate::api::api_reference::aiarena::errors::AiArenaApiError;

use crate::api::api_reference::{ApiError, ControllerApi, ResponseContent};
use crate::models::aiarena::aiarena_match::AiArenaMatch;
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::multipart::Form;
use reqwest::{Client, ClientBuilder, StatusCode, Url};
use tracing::{debug, error, trace};

pub struct AiArenaApiClient {
    client: Client,
    url: Url,
    token: String,
}

impl AiArenaApiClient {
    pub const API_MATCHES_ENDPOINT: &'static str = "/api/arenaclient/matches/";
    pub const API_RESULTS_ENDPOINT: &'static str = "/api/arenaclient/results/";

    pub fn new(website_url: &str, token: &str) -> Result<Self, url::ParseError> {
        let url = Url::parse(website_url)?;

        Ok(Self {
            url,
            client: ClientBuilder::new()
                .timeout(Duration::from_secs(2 * 60))
                .build()
                .unwrap(),
            token: token.to_string(),
        })
    }
    pub async fn get_match(&self) -> Result<AiArenaMatch, ApiError<AiArenaApiError>> {
        // static string, so the constructor should catch any parse errors
        let api_matches_url = self.url.join(Self::API_MATCHES_ENDPOINT).unwrap();

        let request = self
            .client
            .request(reqwest::Method::POST, api_matches_url)
            .header(reqwest::header::AUTHORIZATION, self.token_header())
            .build()?;
        trace!("Sending request: {:?}", request);
        let response = self.client.execute(request).await?;

        let status = response.status();
        let content = response.text().await?;

        if !status.is_client_error() && !status.is_server_error() {
            match serde_json::from_str::<AiArenaMatch>(&content).map_err(ApiError::from) {
                Err(e) => {
                    error!("{}", e);
                    debug!("{}", &content);
                    Err(e)
                }
                e => e,
            }
        } else {
            match serde_json::from_str::<AiArenaApiError>(&content).map_err(ApiError::from) {
                Ok(api_error_message) => {
                    let error = ResponseContent {
                        status,
                        api_error_message,
                    };
                    Err(ApiError::ResponseError(error))
                }
                Err(e) => {
                    error!("status={},error{}", status, e);
                    debug!("{}", &content);
                    Err(e)
                }
            }
        }
    }

    pub async fn download_map(
        &self,
        map_url: &str,
        add_auth_header: bool,
    ) -> Result<Bytes, ApiError<AiArenaApiError>> {
        // static string, so the constructor should catch any parse errors
        let map_url = Url::parse(map_url).map_err(ApiError::from)?;

        let mut request_builder = self.client.request(reqwest::Method::GET, map_url);

        if add_auth_header {
            request_builder =
                request_builder.header(reqwest::header::AUTHORIZATION, self.token_header())
        }
        let request = request_builder.build()?;

        let response = self.client.execute(request).await?;

        let status = response.status();

        if !status.is_client_error() && !status.is_server_error() {
            let content = response.bytes().await?;
            Ok(content)
        } else {
            let content = response.text().await?;
            match serde_json::from_str::<AiArenaApiError>(&content).map_err(ApiError::from) {
                Ok(api_error_message) => {
                    let error = ResponseContent {
                        status,
                        api_error_message,
                    };
                    Err(ApiError::ResponseError(error))
                }
                Err(e) => {
                    error!("status={},error{}", status, e);
                    debug!("{}", &content);
                    Err(e)
                }
            }
        }
    }
    fn token_header(&self) -> String {
        format!("Token {}", &self.token)
    }
    pub async fn download_zip(
        &self,
        url: &str,
        add_auth_header: bool,
    ) -> Result<Bytes, ApiError<AiArenaApiError>> {
        // static string, so the constructor should catch any parse errors
        let url = Url::parse(url).map_err(ApiError::from)?;

        let mut request_builder = self.client.request(reqwest::Method::GET, url);

        if add_auth_header {
            request_builder =
                request_builder.header(reqwest::header::AUTHORIZATION, self.token_header())
        }

        let request = request_builder.build()?;

        let response = self.client.execute(request).await?;

        let status = response.status();

        if !status.is_client_error() && !status.is_server_error() {
            let content = response.bytes().await?;
            Ok(content)
        } else {
            let content = response.text().await?;
            match serde_json::from_str::<AiArenaApiError>(&content).map_err(ApiError::from) {
                Ok(api_error_message) => {
                    let error = ResponseContent {
                        status,
                        api_error_message,
                    };
                    Err(ApiError::ResponseError(error))
                }
                Err(e) => {
                    error!("status={},error{}", status, e);
                    debug!("{}", &content);
                    Err(e)
                }
            }
        }
    }
    pub async fn submit_result(&self, form: Form) -> Result<StatusCode, reqwest::Error> {
        let api_submission_url = self.url.join(Self::API_RESULTS_ENDPOINT).unwrap();
        let request = self
            .client
            .request(reqwest::Method::POST, api_submission_url)
            .multipart(form)
            .header(reqwest::header::AUTHORIZATION, self.token_header())
            .build()
            .unwrap();

        let response = self.client.execute(request).await?;

        let status = response.status();

        if status.is_client_error() || status.is_server_error() {
            error!("{:?}: {:?}", &status, &response.text().await);
        }
        Ok(status)
    }
}

#[async_trait]
impl ControllerApi for AiArenaApiClient {
    const API_TYPE: &'static str = "BotController";

    fn url(&self) -> &Url {
        &self.url
    }

    fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use crate::api::api_reference::aiarena::aiarena_api_client::AiArenaApiClient;
    use crate::api::api_reference::aiarena::AiArenaResultForm;
    use crate::api::api_reference::bot_controller_client::BotController;
    use crate::api::api_reference::ControllerApi;
    use crate::models::aiarena::aiarena_game_result::AiArenaGameResult;
    use crate::models::aiarena::aiarena_result::AiArenaResult;
    use httpmock::prelude::*;
    use httpmock::MockServer;

    #[test_log::test(tokio::test)]
    async fn test_get_match() {
        let mockserver = MockServer::start();
        mockserver.mock(|when, then| {
            when.method(POST)
                .path(AiArenaApiClient::API_MATCHES_ENDPOINT);
            then.status(200)
                .body_from_file("../testing/api-based/responses/get_match_response.json")
                .header("Content-Type", "application/json");
        });

        let api =
            AiArenaApiClient::new(&mockserver.base_url(), "987").expect("Could not create client");
        let new_match = api.get_match().await;
        assert!(new_match.is_ok());
        let new_match = new_match.unwrap();
        assert_eq!(new_match.id, 1);
    }

    #[test_log::test(tokio::test)]
    async fn test_submit_result() {
        let token = "567";
        let mockserver = MockServer::start();
        mockserver.mock(|when, then| {
            when.method(POST)
                .path(AiArenaApiClient::API_RESULTS_ENDPOINT)
                .header("authorization", format!("Token {token}"));
            then.status(200);
        });

        let api =
            AiArenaApiClient::new(&mockserver.base_url(), token).expect("Could not create client");
        let result = AiArenaGameResult {
            match_id: 1,
            bot1_avg_step_time: Some(0.1),
            bot1_tags: Some(vec!["tag1".to_string()]),
            bot2_avg_step_time: None,
            bot2_tags: None,
            result: AiArenaResult::Player1Win,
            game_steps: 10,
        };
        let form = AiArenaResultForm::from(&result);
        let result_post = api.submit_result(form.to_inner()).await;
        assert!(result_post.is_ok());
        assert!(result_post.unwrap().is_success());
    }

    #[test]
    fn test_get_socket_addr() {
        let ip_addr = "127.0.0.1".to_string();
        let port = 8083;
        let bot_controller = BotController::new(&ip_addr, port).expect("Could not parse address");
        let socket_addr = bot_controller.sock_addr();
        assert_eq!(socket_addr.port(), port);
        assert_eq!(socket_addr.ip().to_string(), ip_addr);
    }
}
