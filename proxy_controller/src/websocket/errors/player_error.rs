use axum::extract::ws::Message as AxumMessage;
use std::time::Duration;
use std::{error, fmt};

use tokio_tungstenite::tungstenite::Message as TungsteniteMessage;

#[derive(Debug)]
pub enum PlayerError {
    BotQuit,
    NoMessageAvailable,
    BotWebsocket(axum::Error),
    Sc2Websocket(tokio_tungstenite::tungstenite::Error),
    BotUnexpectedMessage(AxumMessage),
    Sc2UnexpectedMessage(TungsteniteMessage),
    UnexpectedRequest(sc2_proto::sc2api::Request),
    ProtoParseError(protobuf::Error),
    CreateGame(sc2_proto::sc2api::response_create_game::Error),
    JoinGameTimeout(Duration),
    Sc2Timeout(Duration),
    BotTimeout(Duration),
}

impl From<axum::Error> for PlayerError {
    fn from(error: axum::Error) -> Self {
        Self::BotWebsocket(error)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for PlayerError {
    fn from(error: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Sc2Websocket(error)
    }
}

impl From<protobuf::Error> for PlayerError {
    fn from(error: protobuf::Error) -> Self {
        Self::ProtoParseError(error)
    }
}

impl fmt::Display for PlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (module, e) = match self {
            Self::BotQuit => ("BotQuit", "Bot has quit unexpectedly".to_string()),
            Self::NoMessageAvailable => ("NoMessageAvailable", "No message available".to_string()),
            Self::BotWebsocket(e) => ("BotWebsocket", e.to_string()),
            Self::Sc2Websocket(e) => ("Sc2Websocket", e.to_string()),
            Self::BotUnexpectedMessage(e) => (
                "BotUnexpectedMessage",
                format!("Expected Binary message, received: {:?}", e),
            ),
            Self::Sc2UnexpectedMessage(e) => (
                "Sc2UnexpectedMessage",
                format!("Expected Binary message, received: {:?}", e),
            ),
            Self::UnexpectedRequest(e) => (
                "UnexpectedRequest",
                format!("Unexpected request received: {}", e),
            ),
            Self::ProtoParseError(e) => (
                "ProtoParseError",
                format!("Could not parse proto message: {:?}", e),
            ),
            Self::CreateGame(e) => ("CreateGame", format!("Could not create game: {:?}", e)),
            Self::JoinGameTimeout(d) => (
                "JoinGameTimeout",
                format!("Timeout of {:?}s reached while waiting for bot to join", d),
            ),
            Self::Sc2Timeout(d) => (
                "SC2Timeout",
                format!("Timeout of {:?}s while waiting for SC2 communication", d),
            ),
            Self::BotTimeout(d) => (
                "BotTimeout",
                format!("Timeout of {:?}s while waiting for bot communication", d),
            ),
        };
        write!(f, "{}: {}", module, e)
    }
}

impl error::Error for PlayerError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(match self {
            PlayerError::BotWebsocket(e) => e,
            PlayerError::Sc2Websocket(e) => e,
            PlayerError::ProtoParseError(e) => e,
            _ => {
                return None;
            }
        })
    }
}
