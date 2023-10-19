mod model;
pub use model::{EntryType, FileEntry, FileMeta, FileChunk};

use thiserror::Error;
use tokio_tungstenite::tungstenite;

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    /// Invalid command line arguments
    #[error("Invalid command line arguments: {0}")]
    InvalidArgs(String),
    /// Invalid port number
    #[error("Invalid port number: {0}")]
    InvalidPort(u16),
    /// Port in use
    #[error("Port {0} is already in use")]
    PortInUse(u16),
    /// Directory is not a directory
    #[error("Invalid directory: {0}")]
    InvalidDir(String),
    /// Directory is in use
    #[error("Directory {0} is in use by other receiver")]
    DirInUse(String),
    /// Failed to create directory
    #[error("Failed to create directory: {0}")]
    FailedCreateDir(std::io::Error),
    /// Failed to bind to address
    #[error("Failed to bind to address: {0}")]
    FailedBind(std::io::Error),
    /// Failed to open file
    #[error("Failed to open file: {0}")]
    FailedOpenFile(std::io::Error),
    /// Failed to write to file
    #[error("Failed to write to file: {0}")]
    FailedWriteFile(std::io::Error),
    /// Failed to delete file
    #[error("Failed to delete file: {0}")]
    FailedDeleteFile(std::io::Error),
    /// Failed to read file
    #[error("Failed to read file: {0}")]
    FailedReadFile(std::io::Error),
    /// Websocket error
    #[error("Websocket error: {0}")]
    WsError(#[from] tungstenite::error::Error),
    /// Websocket address parse error
    #[error("Websocket address parse error: {0}")]
    WsAddrParseError(#[from] url::ParseError),
    /// Tokio join handle error
    #[error("Tokio join handle error: {0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
}

// TODO: SyncError
