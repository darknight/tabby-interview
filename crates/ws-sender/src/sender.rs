use std::path::Path;
use futures::future::ready;
use futures::StreamExt;
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use ws_common::{Result, AppError, FileMeta, FileChunk, FileEntry, WsRequest, Shutdown};
use walkdir::DirEntry;
use crate::{FILE_CHUNK_SIZE};
use crate::connection::{WsReader, WsWriter};
use crate::handler::WsHandler;

/// Websocket sender
#[derive(Debug, Clone)]
pub struct WsSender {
    from_dir: String,
    _ws_addr: String,
    ws_url: url::Url,
    /// Broadcast shutdown signal to all active connections
    pub shutdown_sender: broadcast::Sender<()>,
}

impl WsSender {
    /// Given `from_dir` and `ws_addr`, create a websocket sender
    ///
    /// `from_dir` must be valid directory
    pub async fn new(from_dir: String, ws_addr: String, shutdown_sender: broadcast::Sender<()>) -> Result<WsSender> {
        // check if `from_dir` is valid directory
        let path = Path::new(&from_dir);
        if !path.is_dir() {
            return Err(AppError::InvalidDir(from_dir.clone()));
        }

        let ws_url = url::Url::parse(&ws_addr)?;

        ready(
            Ok(WsSender {
                from_dir,
                _ws_addr: ws_addr,
                ws_url,
                shutdown_sender,
            })
        ).await
    }

    /// Run the websocket sender
    pub async fn run(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        let (outgoing, incoming) = ws_stream.split();

        let handler = WsHandler::new(
            self.from_dir.clone(),
            WsWriter::new(outgoing),
            WsReader::new(incoming),
            Shutdown::new(self.shutdown_sender.subscribe()),
            Shutdown::new(self.shutdown_sender.subscribe()),
        );

        handler.run().await
    }

    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }
}

/// Compose `CreateFile` message and send it to channel
pub async fn send_create_file_message(tx: Sender<Message>, file_meta: FileMeta) -> Result<()> {
    let message = WsRequest::new_create_file_message(file_meta)?;
    tx.send(message).await.map_err(AppError::TokioSendError)?;

    Ok(())
}

/// Compose `WriteFile` message and send it to channel
pub async fn send_write_file_message(tx: Sender<Message>, file_meta: FileMeta, dir_entry: DirEntry) -> Result<()> {
    if !file_meta.is_file() {
        return Ok(());
    }

    let mut file = tokio::fs::File::open(dir_entry.path()).await
        .map_err(AppError::FailedOpenFile)?;
    let mut buf = vec![0; FILE_CHUNK_SIZE];
    let mut offset = 0u64;

    // read file into buffer and send file entry to channel
    while let Ok(n) = file.read(&mut buf).await {
        if n == 0 {
            break;
        }
        let actual_payload = buf[..n].to_vec();
        let file_chunk = FileChunk::new(offset, actual_payload);
        let file_entry = FileEntry::new(file_meta.clone(), Some(file_chunk));
        let message = WsRequest::new_write_file_message(file_entry)?;
        tx.send(message).await.map_err(AppError::TokioSendError)?;
        // next offset
        offset += n as u64;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // TODO: add tests
}
