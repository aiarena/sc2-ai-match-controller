use crate::api::api_reference::aiarena::errors::AiArenaApiError;
use std::time::Duration;

use crate::api::api_reference::{ApiError, ControllerApi, ResponseContent};
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::multipart::Form;
use reqwest::{Client, ClientBuilder, Url};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, trace};

pub struct AiArenaApiClient {
    client: Client,
    url: Url,
    token: String,
}

impl AiArenaApiClient {
    pub fn new(website_url: &str, token: &str) -> Result<Self, url::ParseError> {
        let url = Url::parse(website_url)?;

        Ok(Self {
            url,
            client: ClientBuilder::new().build().unwrap(),
            token: token.to_string(),
        })
    }

    pub async fn get_etag(&self, url: &str) -> Result<String, ApiError<AiArenaApiError>> {
        let url = Url::parse(url).map_err(ApiError::from)?;
        let request = self.client.request(reqwest::Method::HEAD, url).build()?;
        trace!("Sending HEAD request: {:?}", request);
        let response = self.client.execute(request).await?;
        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        Ok(etag)
    }

    pub async fn download_map(
        &self,
        map_url: &str,
        _add_auth_header: bool,
    ) -> Result<Bytes, ApiError<AiArenaApiError>> {
        let map_url = Url::parse(map_url).map_err(ApiError::from)?;

        let mut request_builder = self.client.request(reqwest::Method::GET, map_url.clone());

        if let Some(host) = map_url.host_str() {
            if host.contains("aiarena.net") {
                request_builder =
                    request_builder.header(reqwest::header::AUTHORIZATION, self.token_header())
            }
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
        _add_auth_header: bool,
    ) -> Result<Bytes, ApiError<AiArenaApiError>> {
        let url = Url::parse(url).map_err(ApiError::from)?;

        let mut request_builder = self.client.request(reqwest::Method::GET, url.clone());
        debug!("{:?}", &url.host_str());

        if let Some(host) = url.host_str() {
            if host.contains("aiarena.net") {
                request_builder =
                    request_builder.header(reqwest::header::AUTHORIZATION, self.token_header())
            }
        }

        let request = request_builder.build()?;

        let response = self.client.execute(request).await?;

        let status = response.status();

        if !status.is_client_error() && !status.is_server_error() {
            let content = response.bytes().await?;
            Ok(content)
        } else {
            let content = response.text().await?;

            debug!(
                "Website:\nUrl:{}\nStatus:{}\nResponse:{}",
                &url, status, content
            );
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
                    Err(e)
                }
            }
        }
    }

    pub async fn download_cached_file(
        &self,
        url: &str,
        source_url: &str,
        unique_key: &str,
        etag: &str,
    ) -> Result<Bytes, ApiError<AiArenaApiError>> {
        let url = Url::parse(url).map_err(ApiError::from)?;

        let json_body = CacheDownloadRequest {
            unique_key: unique_key.to_string(),
            url: source_url.to_string(),
            etag: etag.to_string(),
        };
        let request_builder = self
            .client
            .request(reqwest::Method::POST, url.clone())
            .json(&json_body);

        let request = request_builder.build()?;
        let mut local_var_resp_result;
        let mut counter = 0;
        loop {
            local_var_resp_result = self.client.execute(request.try_clone().unwrap()).await;
            if local_var_resp_result.is_ok() {
                break;
            }
            if counter > 10 {
                break;
            }
            counter += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let response = local_var_resp_result?;

        let status = response.status();

        if !status.is_client_error() && !status.is_server_error() {
            let content = response.bytes().await?;
            Ok(content)
        } else {
            let content = response.text().await?;

            debug!(
                "Website:\nUrl:{}\nStatus:{}\nResponse:{}",
                &url, status, content
            );
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
                    Err(e)
                }
            }
        }
    }

    pub async fn cache_upload(
        &self,
        url: &str,
        unique_key: String,
        file: &[u8],
    ) -> Result<(), ApiError<String>> {
        let mut local_var_resp_result;
        let mut counter = 0;
        loop {
            let mut request_builder = self.client.request(reqwest::Method::POST, url);
            request_builder = request_builder.query(&[("uniqueKey", &unique_key.to_string())]);
            let mut local_var_form = Form::new();
            let part = reqwest::multipart::Part::bytes(file.to_vec()).file_name(unique_key.clone());
            local_var_form = local_var_form.part("file", part);

            request_builder = request_builder.multipart(local_var_form);
            let local_var_req = request_builder.build()?;
            local_var_resp_result = self.client.execute(local_var_req).await;
            if local_var_resp_result.is_ok() {
                break;
            }
            if counter > 10 {
                break;
            }
            counter += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        let local_var_resp = local_var_resp_result?;

        let local_var_status = local_var_resp.status();
        let local_var_content = local_var_resp.text().await?;

        if !local_var_status.is_client_error() && !local_var_status.is_server_error() {
            Ok(())
        } else {
            error!("{:?}: {:?}", &local_var_status, &local_var_content);
            let error = ResponseContent {
                status: local_var_status,
                api_error_message: local_var_content,
            };
            Err(ApiError::ResponseError(error))
        }
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CacheDownloadRequest {
    #[serde(rename = "uniqueKey")]
    pub unique_key: String,
    pub url: String,
    #[serde(rename = "md5hash")] // cache server API uses "md5hash" as the key name
    pub etag: String,
}
