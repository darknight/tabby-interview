use std::path::Path;
use futures::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use ws_common::{Result, AppError, FileMeta, FileChunk, FileEntry, WsRequest, WsResponse, walk_dir};
use walkdir::DirEntry;
use crate::{CHANNEL_CAPACITY, FILE_CHUNK_SIZE};

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
        let (tx, mut rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);

        // spawn blocking task to walk directory
        let meta_infos = tokio::task::spawn_blocking(move || {
            walk_dir(from_dir, false)
        }).await?;

        // FIXME: send clear dir message in separate task
        // let file_metas = meta_infos.keys().cloned().collect::<Vec<FileMeta>>();
        // let message = WsRequest::new_clear_dir_message(file_metas)?;
        // outgoing.send(message).await?;

        // spawn a task to accept file entry from channel and send them to receiver
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                debug!("[Sender] send ws request");
                if let Err(err) = outgoing.send(msg).await {
                    error!("[Sender] failed to send ws request: {}", err);
                }
            }
        });

        // read message from incoming stream
        while let Some(raw) = incoming.next().await {
            match raw {
                Ok(msg) => {
                    match msg {
                        Message::Text(text) => {
                            let ws_resp = serde_json::from_str::<WsResponse>(&text);
                            if ws_resp.is_err() {
                                error!("[Sender] failed to parse ws response: {}", text);
                                continue;
                            }
                            let ws_resp = ws_resp.unwrap();
                            match ws_resp {
                                WsResponse::CreateSuccess(file_meta) => {
                                    info!("[Sender] create file done: {:?}", file_meta);
                                    if file_meta.is_file() {
                                        if let Some(dir_entry) = meta_infos.get(&file_meta) {
                                            let tx = tx.clone();
                                            let dir_entry = dir_entry.clone();
                                            tokio::spawn(async move {
                                                if let Err(err) = send_write_file_message(tx, file_meta, dir_entry.clone()).await {
                                                    error!("[Sender] create file entry: {}", err);
                                                }
                                            });
                                        }
                                    }
                                }
                                WsResponse::CreateFailed(file_meta) => {
                                    error!("[Sender] create file failed: {:?}", file_meta);
                                }
                                WsResponse::WriteSuccess(file_meta) => {
                                    info!("[Sender] receiver write done: {:?}", file_meta);
                                }
                                WsResponse::WriteFailed(file_meta) => {
                                    error!("[Sender] receiver write failed: {:?}", file_meta);
                                    // TODO: retry?
                                }
                                WsResponse::ClearDirDone(_) => {
                                    info!("[Sender] receiver dir is ready");
                                    // spawn task to send create file message
                                    meta_infos.clone().into_iter().for_each(|(file_meta, _)| {
                                        let tx = tx.clone();
                                        tokio::spawn(async move {
                                            if let Err(err) = send_create_file_message(tx, file_meta).await {
                                                error!("[Sender] send create file message: {}", err);
                                            }
                                        });
                                    });
                                },
                            }
                        },
                        Message::Close(_) => {
                            warn!("[Sender] connection is closed by receiver");
                            return Ok(());
                        },
                        _ => {},
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

/// Compose `CreateFile` message and send it to channel
async fn send_create_file_message(tx: Sender<Message>, file_meta: FileMeta) -> Result<()> {
    let message = WsRequest::new_create_file_message(file_meta)?;
    tx.send(message).await.map_err(AppError::TokioSendError)?;

    Ok(())
}

/// Compose `WriteFile` message and send it to channel
async fn send_write_file_message(tx: Sender<Message>, file_meta: FileMeta, dir_entry: DirEntry) -> Result<()> {
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
}
