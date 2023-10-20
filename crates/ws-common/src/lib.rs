mod model;

use log::{debug, error, warn};
pub use model::{EntryType, FileEntry, FileMeta, FileChunk, WsRequest, WsResponse};

use thiserror::Error;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite;
use walkdir::{DirEntry, WalkDir};

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
    TokioSendError(#[from] mpsc::error::SendError<FileEntry>),

    /// Serde_json error
    #[error("Serde_json error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
}

/// Walk directory and return file meta infos
pub fn walk_dir(base_dir: String, contain_symlink: bool) -> Vec<(FileMeta, DirEntry)> {
    let mut meta_infos = Vec::new();

    // the first item yielded by `WalkDir` is the root directory itself, so we skip it
    for dir_entry in WalkDir::new(base_dir.as_str()).into_iter().skip(1) {
        if let Err(err) = dir_entry {
            error!("[Sender] walk dir error: {}", err);
            continue;
        }
        let dir_entry = dir_entry.unwrap();

        let entry_type = if dir_entry.file_type().is_dir() { EntryType::Dir } else if dir_entry.file_type().is_file() { EntryType::File } else { EntryType::SymLink };

        // since the entry is from `from_dir`, we can safely unwrap here
        let rel_path = dir_entry.path().strip_prefix(base_dir.as_str()).unwrap().to_str();
        if rel_path.is_none() {
            warn!("[Sender] invalid rel path: {:?}", rel_path);
            continue;
        }
        let rel_path = rel_path.unwrap().to_string();
        let file_meta = FileMeta::new(rel_path, entry_type.clone());
        debug!("file meta: {:?}", file_meta);
        if !contain_symlink && entry_type == EntryType::SymLink {
            continue;
        }
        meta_infos.push((file_meta, dir_entry));
    }
    meta_infos
}
