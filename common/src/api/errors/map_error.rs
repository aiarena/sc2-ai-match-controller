use std::io::{Error, ErrorKind};

/// Errors that can happen interacting with processes.
#[derive(Debug)]
pub enum MapError {
    NotFound(Error),
    Other(Error),
}

impl From<Error> for MapError {
    fn from(err: Error) -> Self {
        match err.kind() {
            ErrorKind::NotFound => Self::NotFound(err),
            _ => Self::Other(err),
        }
    }
}
