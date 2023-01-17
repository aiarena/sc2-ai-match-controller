use std::io;
use std::io::Error;
use zip::result::ZipError;

/// Errors that can happen while downloading files.
#[derive(Debug)]
pub enum DownloadError {
    TempFile(Error),
    FileNotFound(Error),
    BotFolderNotFound(String),
    Unauthorized,
    Io(Error),
    ZipError(ZipError),
    NotAvailable(String),
    Other(String),
}

impl From<io::Error> for DownloadError {
    fn from(err: Error) -> Self {
        DownloadError::Io(err)
    }
}
impl From<ZipError> for DownloadError {
    fn from(err: ZipError) -> Self {
        DownloadError::ZipError(err)
    }
}
