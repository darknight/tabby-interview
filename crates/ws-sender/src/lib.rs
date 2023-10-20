use std::collections::BTreeSet;
use std::os::windows::fs::FileTypeExt;
use std::path::Path;
use futures::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, MaybeTlsStream, tungstenite, WebSocketStream};
use ws_common::{Result, AppError, EntryType, FileMeta, FileChunk, FileEntry, WsRequest, WsResponse};
use walkdir::{DirEntry, WalkDir};

const CHANNEL_CAPACITY: usize = 10;

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
        info!("[Sender] from directory: {}", from_dir);
        let (mut outgoing, mut incoming) = self.ws_stream.split();

        // create channel to collect file entry from tasks
        let (tx, mut rx) = mpsc::channel::<FileEntry>(CHANNEL_CAPACITY);

        // spawn blocking task to walk directory
        let meta_infos = tokio::task::spawn_blocking(move || {
            walk_dir(from_dir)
        }).await?;

        let file_metas = meta_infos.iter().map(|meta| meta.0.clone()).collect::<Vec<FileMeta>>();
        let message = WsRequest::new_list_dir_message(file_metas)?;
        outgoing.send(message).await?;

        // spawn a task to accept file entry from channel and send them to receiver
        tokio::spawn(async move {
            while let Some(file_entry) = rx.recv().await {
                info!("[Sender] prepare to send file entry: {:?}", file_entry);
                match WsRequest::new_write_file_message(file_entry) {
                    Ok(message) => {
                        if let Err(err) = outgoing.send(message).await {
                            error!("[Sender] failed to send ws message: {}", err);
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
                                error!("[Sender] failed to parse message: {}", text);
                                continue;
                            }
                            let ws_resp = ws_resp.unwrap();
                            debug!("[Sender] received message: {:?}", ws_resp);
                            match ws_resp {
                                WsResponse::WriteSuccess(file_meta) => {
                                    debug!("[Sender] successfully write file on receiver side: {:?}", file_meta);
                                }
                                WsResponse::WriteFailed(file_meta) => {
                                    error!("[Sender] failed to write file on receiver side: {:?}", file_meta);
                                }
                                WsResponse::DeleteDone(failed) => {
                                    // skip failed files, do not send them
                                    let failed_set = failed.iter().map(|meta| meta.rel_path().to_string()).collect::<BTreeSet<String>>();
                                    meta_infos.clone().into_iter().filter(|meta| !failed_set.contains(meta.0.rel_path())).for_each(|meta| {
                                        tokio::spawn(create_file_entry(tx.clone(), meta.1, meta.0));
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

fn walk_dir(from_dir: String) -> Vec<(FileMeta, DirEntry)> {
    let mut meta_infos = Vec::new();

    // the first item yielded by `WalkDir` is the root directory itself, so we skip it
    for dir_entry in WalkDir::new(from_dir.as_str()).into_iter().skip(1) {
        if let Err(err) = dir_entry {
            error!("[Sender] walk dir error: {}", err);
            continue;
        }
        let dir_entry = dir_entry.unwrap();

        let entry_type = if dir_entry.file_type().is_dir() { EntryType::Dir } else if dir_entry.file_type().is_file() { EntryType::File } else { EntryType::SymLink };

        // since the entry is from `from_dir`, we can safely unwrap here
        let rel_path = dir_entry.path().strip_prefix(from_dir.as_str()).unwrap().to_str();
        if rel_path.is_none() {
            warn!("[Sender] invalid rel path: {:?}", rel_path);
            continue;
        }
        let rel_path = rel_path.unwrap().to_string();
        let file_meta = FileMeta::new(rel_path, entry_type.clone());
        debug!("file meta: {:?}", file_meta);
        if entry_type != EntryType::SymLink {
            // save file meta for comparing with receiver's files
            // ignore symlink on purpose
            meta_infos.push((file_meta, dir_entry));
        }
    }
    meta_infos
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
