use std::path::Path;
use futures::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, MaybeTlsStream, tungstenite, WebSocketStream};
use ws_common::{Result, AppError, EntryType, FileMeta, FileChunk, FileEntry, WsRequest, WsResponse, walk_dir};
use walkdir::DirEntry;
use crate::CHANNEL_CAPACITY;

/// Websocket sender
#[derive(Debug, Clone)]
pub struct WsSender {
    from_dir: String,
    ws_addr: String,
    ws_url: url::Url,
}

/// Websocket stream
#[derive(Debug)]
pub struct WsStream {
    from_dir: String,
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl WsSender {
    /// Given `from_dir` and `ws_addr`, create a websocket sender
    ///
    /// `from_dir` must be valid directory
    pub fn new(from_dir: String, ws_addr: String) -> Result<WsSender> {
        // check if `from_dir` is valid directory
        let path = Path::new(&from_dir);
        if !path.is_dir() {
            return Err(AppError::InvalidDir(from_dir.clone()));
        }

        let ws_url = url::Url::parse(&ws_addr)?;

        Ok(WsSender {
            from_dir,
            ws_addr,
            ws_url,
        })
    }

    /// Connect to websocket server, return websocket stream
    pub async fn connect(&self) -> Result<WsStream> {
        let (ws_stream, _) = connect_async(&self.ws_url).await?;
        Ok(WsStream {
            from_dir: self.from_dir.clone(),
            ws_stream,
        })
    }
}

impl WsStream {
    /// Sync directory via websocket stream
    pub async fn sync_dir(self) -> Result<()> {
        let from_dir = self.from_dir.clone();
        info!("[Sender] base directory: {}", from_dir);
        let (mut outgoing, mut incoming) = self.ws_stream.split();

        // create channel to collect file entry from tasks
        let (tx, mut rx) = mpsc::channel::<FileEntry>(CHANNEL_CAPACITY);

        // spawn blocking task to walk directory
        let meta_infos = tokio::task::spawn_blocking(move || {
            walk_dir(from_dir, false)
        }).await?;

        let file_metas = meta_infos.iter().map(|meta| meta.0.clone()).collect::<Vec<FileMeta>>();
        let message = WsRequest::new_clear_dir_message(file_metas)?;
        outgoing.send(message).await?;

        // spawn a task to accept file entry from channel and send them to receiver
        tokio::spawn(async move {
            while let Some(file_entry) = rx.recv().await {
                info!("[Sender] send file entry: {:?}", file_entry);
                match WsRequest::new_write_file_message(file_entry) {
                    Ok(message) => {
                        if let Err(err) = outgoing.send(message).await {
                            error!("[Sender] failed to send file entry: {}", err);
                        }
                    }
                    Err(err) => {
                        error!("[Sender] failed to create ws message {:?}", err);
                    }
                }
            }
        });

        // read message from incoming stream
        while let Some(raw) = incoming.next().await {
            match raw {
                Ok(msg) => {
                    match msg {
                        tungstenite::Message::Text(text) => {
                            let ws_resp = serde_json::from_str::<WsResponse>(&text);
                            if ws_resp.is_err() {
                                error!("[Sender] failed to parse ws response: {}", text);
                                continue;
                            }
                            let ws_resp = ws_resp.unwrap();
                            match ws_resp {
                                WsResponse::WriteSuccess(file_meta) => {
                                    info!("[Sender] receiver write done: {:?}", file_meta);
                                }
                                WsResponse::WriteFailed(file_meta) => {
                                    error!("[Sender] receiver write failed: {:?}", file_meta);
                                }
                                WsResponse::ClearDirDone(_) => {
                                    info!("[Sender] receiver dir is ready");
                                    // spawn task to create file entry
                                    meta_infos.clone().into_iter().for_each(|(file_meta, dir_entry)| {
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            if let Err(err) = create_file_entry(tx, dir_entry, file_meta).await {
                                                error!("[Sender] failed to create file entry: {:?}", err);
                                            }
                                        });
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(err) => {
                    error!("[Sender] read message error: {}", err);
                }
            }
        }

        Ok(())
    }
}

async fn create_file_entry(tx: Sender<FileEntry>, dir_entry: DirEntry, file_meta: FileMeta) -> Result<()> {
    let file_chunk = match file_meta.entry_type() {
        EntryType::File => {
            let mut file = tokio::fs::File::open(dir_entry.path()).await
                .map_err(AppError::FailedOpenFile)?;
            let mut file_chunk = Vec::new();
            file.read_to_end(&mut file_chunk).await
                .map_err(AppError::FailedReadFile)?;
            Some(FileChunk::new(0, file_chunk))
        }
        _ => None,
    };
    let file_entry = FileEntry::new(file_meta, file_chunk);
    tx.send(file_entry).await.map_err(AppError::TokioSendError)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
}
