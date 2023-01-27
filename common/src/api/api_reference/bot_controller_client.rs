use crate::api::api_reference::{ApiError, ControllerApi};
use crate::api::errors::app_error::ApiErrorMessage;
use crate::models::bot_controller::StartBot;
use crate::models::StartResponse;
use crate::portpicker::Port;
use async_trait::async_trait;
use bytes::Bytes;
use reqwest::{Client, Url};

#[derive(Debug, Clone)]
pub struct BotController {
    client: Client,
    url: Url,
    process_key: Port,
    start_bot: Option<StartBot>,
}

impl BotController {
    pub fn new(host: &str, port: Port) -> Result<Self, url::ParseError> {
        let url_string = format!("http://{host}:{port}");
        let url = Url::parse(&url_string)?;

        Ok(Self {
            url,
            client: Client::new(),
            process_key: 0,
            start_bot: None,
        })
    }

    pub fn set_process_key(&mut self, process_key: Port) {
        self.process_key = process_key
    }

    pub fn set_start_bot(&mut self, start_bot: StartBot) {
        self.start_bot = Some(start_bot)
    }

    pub async fn start(&self) -> Result<StartResponse, ApiError<ApiErrorMessage>> {
        let start_url = self.url.join("/start").unwrap(); // static string, so the constructor should catch any parse
                                                          // errors
        let request = self
            .client
            .request(reqwest::Method::POST, start_url)
            .json(&self.start_bot)
            .build()?;
        self.execute_request(request).await
    }

    pub async fn download_bot_log(&self) -> Result<Bytes, ApiError<ApiErrorMessage>> {
        let path = format!(
            "/download/bot/{}/log",
            urlencoding::encode(&self.start_bot.as_ref().unwrap().bot_name)
        );
        let log_url = self.url.join(&path).unwrap(); // static string, so the constructor should catch any parse
                                                     // errors
        let request = self.client.request(reqwest::Method::GET, log_url).build()?;

        self.execute_request_file(request).await
    }

    pub async fn download_bot_data(&self) -> Result<Bytes, ApiError<ApiErrorMessage>> {
        let path = format!(
            "/download/bot/{}/data",
            urlencoding::encode(&self.start_bot.as_ref().unwrap().bot_name)
        );
        let log_url = self.url.join(&path).unwrap(); // static string, so the constructor should catch any parse
                                                     // errors
        let request = self.client.request(reqwest::Method::GET, log_url).build()?;

        self.execute_request_file(request).await
    }
}

#[async_trait]
impl ControllerApi for BotController {
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
    use crate::api::api_reference::bot_controller_client::BotController;
    use crate::api::api_reference::ControllerApi;

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
