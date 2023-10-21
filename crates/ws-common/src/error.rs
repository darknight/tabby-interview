use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite;

pub type Result<T> = std::result::Result<T, AppError>;

/// Global application error
#[derive(Debug, Error)]
pub enum AppError {
    /// Invalid command line arguments
    #[error("Invalid command line arguments: {0}")]
    InvalidArgs(String),
    /// Invalid port number
    #[error("System reserved port: {0}")]
    SystemReservedPort(u16),
    /// Port in use
    #[error("Port {0} is already in use")]
    PortInUse(u16),
    /// Directory is not a directory
    #[error("Invalid directory: {0}")]
    InvalidDir(String),
    /// Directory is in use
    #[error("Directory {0} is in use by other receiver")]
    DirInUse(String),
    /// Empty payload
    #[error("Empty payload")]
    EmptyPayload,
    /// File not exist
    #[error("File not exist: {0}")]
    FileNotExist(String),

    // ----------------------std io error----------------------
    /// Failed to create directory
    #[error("Failed to create directory: {0}")]
    FailedCreateDir(std::io::Error),
    /// Failed to delete directory
    #[error("Failed to delete directory: {0}")]
    FailedDeleteDir(std::io::Error),
    /// Failed to read directory
    #[error("Failed to read directory: {0}")]
    FailedReadDir(std::io::Error),
    /// DirEntry error
    #[error("DirEntry error: {0}")]
    DirEntryError(std::io::Error),
    /// Failed to bind to address
    #[error("Failed to bind to address: {0}")]
    FailedBind(std::io::Error),
    /// Failed to create file
    #[error("Failed to create file: {0}")]
    FailedCreateFile(std::io::Error),
    /// Failed to open file
    #[error("Failed to open file: {0}")]
    FailedOpenFile(std::io::Error),
    /// Failed to seek file
    #[error("Failed to seek file: {0}")]
    FailedSeekFile(std::io::Error),
    /// Failed to write to file
    #[error("Failed to write to file: {0}")]
    FailedWriteFile(std::io::Error),
    /// Failed to delete file
    #[error("Failed to delete file: {0}")]
    FailedDeleteFile(std::io::Error),
    /// Failed to read file
    #[error("Failed to read file: {0}")]
    FailedReadFile(std::io::Error),

    // ----------------------tokio & websocket error----------------------
    /// Websocket error
    #[error("Websocket error: {0}")]
    WsError(#[from] tungstenite::error::Error),
    /// Websocket address parse error
    #[error("Websocket address parse error: {0}")]
    WsAddrParseError(#[from] url::ParseError),
    /// Tokio join handle error
    #[error("Tokio join handle error: {0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
    /// Tokio send error
    #[error("Tokio send error: {0}")]
    TokioSendError(#[from] mpsc::error::SendError<tungstenite::Message>),

    /// Serde_json error
    #[error("Serde_json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}
