use crate::api::errors::app_error::ApiErrorMessage;
use crate::models::stats::{HostStats, ProcessStats};
use crate::models::ProcessStatusResponse;
use crate::utilities::portpicker::Port;
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::{Client, Request, Url};
use tracing::error;

use serde::de::DeserializeOwned;
use std::fmt::{Debug, Display};
use std::net::SocketAddr;
use std::{error, fmt};

pub mod aiarena;
pub mod bot_controller_client;
pub mod sc2_controller_client;

#[async_trait]
pub trait ControllerApi {
    const API_TYPE: &'static str;

    fn url(&self) -> &Url;
    fn client(&self) -> &Client;

    async fn health(&self) -> bool {
        let health = self.url().join("/health").unwrap(); // static string, so the constructor should catch any parse
                                                          // errors

        self.client()
            .get(health)
            .send()
            .await
            .ok()
            .map_or(false, |x| x.status().is_success())
    }

    async fn health_with_retry(&self, max_retries: i32) -> bool {
        let mut retries = 0;
        while retries <= max_retries {
            let health_url = self.url().join("/health").unwrap();
            // static string, so the constructor should catch any parse
            // errors
            tracing::debug!(
                api = Self::API_TYPE,
                url = health_url.as_str(),
                "Calling health endpoint"
            );
            let response = self.client().get(health_url).send().await;

            if let Ok(resp) = response {
                if !resp.status().is_success() {
                    retries += 1;
                    continue;
                }
                return true;
            }
            retries += 1;
            continue;
        }
        false
    }

    async fn stats(&self, port: Port) -> Result<ProcessStats, ApiError<ApiErrorMessage>> {
        let key = port.to_string();
        let stats_url = self
            .url()
            .join("/stats/")
            .map(|url| url.join(&key))
            .unwrap()
            .unwrap();
        // static string, so the constructor should catch any parse
        // errors
        tracing::debug!(
            api = Self::API_TYPE,
            url = stats_url.as_str(),
            "Calling stats endpoint"
        );
        Ok(self.client().get(stats_url).send().await?.json().await?)
    }

    async fn stats_host(&self) -> Result<HostStats, ApiError<ApiErrorMessage>> {
        let stats_url = self.url().join("/stats/host").unwrap();
        // static string, so the constructor should catch any parse
        // errors
        tracing::debug!(
            api = Self::API_TYPE,
            url = stats_url.as_str(),
            "Calling stats_host endpoint"
        );
        Ok(self.client().get(stats_url).send().await?.json().await?)
    }

    async fn status(&self, port: Port) -> Result<ProcessStatusResponse, ApiError<ApiErrorMessage>> {
        let key = port.to_string();
        let status_url = self
            .url()
            .join("/status/")
            .map(|url| url.join(&key))
            .unwrap()
            .unwrap();
        // static string, so the constructor should catch any parse
        // errors
        tracing::debug!(
            api = Self::API_TYPE,
            url = status_url.as_str(),
            "Calling status endpoint"
        );
        Ok(self.client().get(status_url).send().await?.json().await?)
    }

    fn sock_addr(&self) -> SocketAddr {
        self.url().socket_addrs(|| None).unwrap()[0]
    }

    async fn execute_request<T>(&self, request: Request) -> Result<T, ApiError<ApiErrorMessage>>
    where
        T: DeserializeOwned,
    {
        let response = self.client().execute(request).await?;

        let status = response.status();
        let content = response.text().await?;

        if !status.is_client_error() && !status.is_server_error() {
            match serde_json::from_str::<T>(&content).map_err(ApiError::from) {
                Err(e) => {
                    tracing::error!("{}", e);
                    tracing::debug!("{}", &content);
                    Err(e)
                }
                e => e,
            }
        } else {
            match serde_json::from_str::<ApiErrorMessage>(&content).map_err(ApiError::from) {
                Ok(api_error_message) => {
                    let error = ResponseContent {
                        status,
                        api_error_message,
                    };
                    let err = ApiError::ResponseError(error);
                    error!("{:?}", err);
                    Err(err)
                }
                Err(e) => {
                    tracing::error!("status={},error{}", status, e);
                    tracing::debug!("{}", &content);
                    Err(e)
                }
            }
        }
    }

    async fn execute_request_file(
        &self,
        request: Request,
    ) -> Result<Bytes, ApiError<ApiErrorMessage>> {
        let response = self.client().execute(request).await?;

        let status = response.status();

        if !status.is_client_error() && !status.is_server_error() {
            let content = response.bytes().await?;
            Ok(content)
        } else {
            let content = response.text().await?;
            match serde_json::from_str::<ApiErrorMessage>(&content).map_err(ApiError::from) {
                Ok(api_error_message) => {
                    let error = ResponseContent {
                        status,
                        api_error_message,
                    };
                    let err = ApiError::ResponseError(error);
                    error!("{:?}", err);
                    Err(err)
                }
                Err(e) => {
                    tracing::error!("status={},error{}", status, e);
                    tracing::debug!("{}", &content);
                    Err(e)
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum ApiError<T> {
    Reqwest(reqwest::Error),
    Url(url::ParseError),
    Serde(serde_json::Error),
    Io(std::io::Error),
    ResponseError(ResponseContent<T>),
    AnyhowError(anyhow::Error),
}

impl<T> Display for ApiError<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (module, e) = match self {
            Self::Reqwest(e) => ("reqwest", e.to_string()),
            Self::Serde(e) => ("serde", e.to_string()),
            Self::Io(e) => ("IO", e.to_string()),
            Self::ResponseError(e) => (
                "response",
                format!("status code {}\nError:{:?}", e.status, e.api_error_message),
            ),
            Self::Url(e) => ("url", e.to_string()),
            Self::AnyhowError(e) => ("anyhow", e.to_string()),
        };
        write!(f, "error in {module}: {e}")
    }
}

impl<T> error::Error for ApiError<T>
where
    T: Debug,
{
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(match self {
            Self::Reqwest(e) => e,
            Self::Serde(e) => e,
            Self::Io(e) => e,
            Self::ResponseError(_) => return None,
            Self::Url(e) => e,
            Self::AnyhowError(e) => e.as_ref(),
        })
    }
}

impl<T> From<reqwest::Error> for ApiError<T> {
    fn from(e: reqwest::Error) -> Self {
        Self::Reqwest(e)
    }
}

impl<T> From<serde_json::Error> for ApiError<T> {
    fn from(e: serde_json::Error) -> Self {
        Self::Serde(e)
    }
}
impl<T> From<anyhow::Error> for ApiError<T> {
    fn from(e: anyhow::Error) -> Self {
        Self::AnyhowError(e)
    }
}

impl<T> From<std::io::Error> for ApiError<T> {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl<T> From<url::ParseError> for ApiError<T> {
    fn from(e: url::ParseError) -> Self {
        Self::Url(e)
    }
}

#[derive(Debug, Clone)]
pub struct ResponseContent<T> {
    pub status: reqwest::StatusCode,
    pub api_error_message: T,
}
