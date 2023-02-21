use std::io;
use std::io::Error;

/// Errors that can happen while downloading files.
#[derive(Debug)]
pub enum DownloadError {
    TempFile(Error),
    FileNotFound(Error),
    BotFolderNotFound(String),
    Unauthorized,
    Io(Error),
    ZipError(anyhow::Error),
    NotAvailable(String),
    Other(String),
}

impl From<io::Error> for DownloadError {
    fn from(err: Error) -> Self {
        DownloadError::Io(err)
    }
}

impl From<anyhow::Error> for DownloadError {
    fn from(err: anyhow::Error) -> Self {
        DownloadError::ZipError(err)
    }
}
