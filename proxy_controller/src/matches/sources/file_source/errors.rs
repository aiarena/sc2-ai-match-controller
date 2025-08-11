use common::models::aiarena::aiarena_match::SerializationError;
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug)]
pub enum FileMatchExtractError {
    PlayerId(Vec<String>),
    PlayerName(Vec<String>),
    PlayerRace(Vec<String>),
    PlayerType(Vec<String>),
    MapName(Vec<String>),
    TooManyFields(Vec<String>),
    MissingFields(Vec<String>),
}

impl Display for FileMatchExtractError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (e, vec) = match self {
            Self::PlayerId(vec) => ("Could not extract player 1 ID", vec),
            Self::PlayerName(vec) => ("Could not extract player 1 name", vec),
            Self::PlayerRace(vec) => ("Could not extract player 1 race", vec),
            Self::MapName(vec) => ("Could not extract map", vec),
            Self::TooManyFields(vec) => ("Too many fields in line", vec),
            Self::MissingFields(vec) => ("Not enough fields in line", vec),
            Self::PlayerType(vec) => ("Could not extract player 1 type", vec),
        };
        write!(f, "{e} in {vec:?}")
    }
}

impl From<SerializationError> for FileMatchExtractError {
    fn from(_error: SerializationError) -> Self {
        Self::MissingFields(Vec::new())
    }
}

impl std::error::Error for FileMatchExtractError {}

#[derive(Debug)]
pub enum SubmissionError {
    FileCreate(std::io::Error),
    FileOpen(std::io::Error),
    FileRead(std::io::Error),
    FileWrite(std::io::Error),
    Serialization(serde_json::Error),
    Truncate(std::io::Error),
    Seek(std::io::Error),
    LogsAndReplaysNull,
}

impl Display for SubmissionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let (explanation, error) = match self {
            SubmissionError::FileCreate(e) => ("Error while creating file", e.to_string()),
            SubmissionError::FileOpen(e) => ("Error while opening file", e.to_string()),
            SubmissionError::FileRead(e) => ("Error while reading file", e.to_string()),
            SubmissionError::FileWrite(e) => ("Error while writing to file", e.to_string()),
            SubmissionError::Serialization(e) => ("Error while serializing results", e.to_string()),
            SubmissionError::Truncate(e) => ("Error while truncating file", e.to_string()),
            SubmissionError::Seek(e) => ("Error while setting cursor on file", e.to_string()),
            SubmissionError::LogsAndReplaysNull => (
                "Error while reading LogsAndReplays Struct",
                "NULL".to_string(),
            ),
        };
        write!(f, "{explanation:?}: {error:?}")
    }
}

impl std::error::Error for SubmissionError {}
