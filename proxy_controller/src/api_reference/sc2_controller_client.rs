use async_trait::async_trait;
use common::api::errors::app_error::ApiErrorMessage;
use common::models::bot_controller::MapData;
use common::models::StartResponse;
use common::portpicker::Port;
use reqwest::{Client, Url};

use crate::api_reference::{ApiError, ControllerApi};

pub struct SC2Controller {
    client: Client,
    url: Url,
    process_key: Port,
}

impl SC2Controller {
    pub fn new(host: &str, port: Port) -> Result<Self, url::ParseError> {
        let url_string = format!("http://{}:{}", host, port);
        let url = Url::parse(&url_string)?;

        Ok(Self {
            url,
            client: Client::new(),
            process_key: 0,
        })
    }
    pub fn set_process_key(&mut self, process_key: Port) {
        self.process_key = process_key
    }

    pub async fn start(&self, map_name: &str) -> Result<StartResponse, ApiError<ApiErrorMessage>> {
        let start_url = self.url.join("/start").unwrap(); // static string, so the constructor should catch any parse
                                                          // errors

        let request = self
            .client
            .request(reqwest::Method::POST, start_url)
            .json(map_name)
            .build()?;

        self.execute_request(request).await
    }

    pub async fn find_map(&self, map_name: &str) -> Result<MapData, ApiError<ApiErrorMessage>> {
        let path = format!("/find_map/{}", map_name);
        let map_url = self.url.join(&path)?;

        let request = self.client.request(reqwest::Method::GET, map_url).build()?;

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
