use crate::api::errors::app_error::ApiErrorMessage;
use crate::models::StartResponse;
use crate::portpicker::Port;
use async_trait::async_trait;
use reqwest::{Client, ClientBuilder, Url};

use crate::api::api_reference::{ApiError, ControllerApi};

#[derive(Debug, Clone)]
pub struct SC2Controller {
    client: Client,
    url: Url,
    process_key: Port,
}

impl SC2Controller {
    pub fn new(host: &str, port: Port) -> Result<Self, url::ParseError> {
        let url_string = format!("http://{host}:{port}");
        let url = Url::parse(&url_string)?;

        Ok(Self {
            url,
            client: ClientBuilder::new().build().unwrap(),
            process_key: 0,
        })
    }
    pub fn set_process_key(&mut self, process_key: Port) {
        self.process_key = process_key
    }

    pub async fn start(&self) -> Result<Vec<StartResponse>, ApiError<ApiErrorMessage>> {
        let start_url = self.url.join("/start").unwrap(); // static string, so the constructor should catch any parse
                                                          // errors

        let request = self
            .client
            .request(reqwest::Method::POST, start_url)
            .build()?;

        self.execute_request(request).await
    }
}

#[async_trait]
impl ControllerApi for SC2Controller {
    const API_TYPE: &'static str = "SC2Controller";

    fn url(&self) -> &Url {
        &self.url
    }

    fn client(&self) -> &Client {
        &self.client
    }
}
