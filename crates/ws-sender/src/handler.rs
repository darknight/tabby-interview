use std::collections::BTreeMap;
use log::{debug, error, info};
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::tungstenite::Message;
use walkdir::DirEntry;
use ws_common::{FileMeta, Shutdown, FileChunk, FileEntry, WsRequest, WsResponse, AppError};
use crate::connection::{WsReader, WsWriter};
use ws_common::Result;
use crate::{FILE_CHUNK_SIZE, CHANNEL_CAPACITY};
use crate::fileio::walk_dir;

/// Handler for sender
pub(crate) struct WsHandler {
    from_dir: String,
    ws_writer: WsWriter,
    ws_reader: WsReader,
    shutdown_for_writer: Shutdown,
    shutdown_for_reader: Shutdown,
}

impl WsHandler {

    /// Create a new websocket handler
    pub fn new(from_dir: String,
               ws_writer: WsWriter,
               ws_reader: WsReader,
               shutdown_for_writer: Shutdown,
               shutdown_for_reader: Shutdown) -> Self {
        Self {
            from_dir,
            ws_writer,
            ws_reader,
            shutdown_for_writer,
            shutdown_for_reader,
        }
    }

    /// Run the websocket handler
    ///
    /// Internally, it spawns two tasks, one for sending requests and the other for receiving responses.
    pub async fn run(self) -> Result<()> {
        let WsHandler {
            from_dir,
            mut ws_writer,
            ws_reader,
            shutdown_for_writer,
            shutdown_for_reader,
        } = self;
        // create channel to collect file entry from tasks
        let (tx, rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);

        // spawn blocking task to walk directory
        info!("[Sender] base directory: {}", from_dir);
        let meta_infos = tokio::task::spawn_blocking(move || {
            walk_dir(from_dir, false)
        }).await?;

        let file_metas = meta_infos.keys().cloned().collect::<Vec<FileMeta>>();
        let message = WsRequest::new_clear_dir_message(file_metas)?;
        ws_writer.write_message(message).await?;

        // run sending task
        let sending = tokio::spawn(async move {
            if let Err(err) = run_for_sending(ws_writer, rx, shutdown_for_writer).await {
                error!("[Sender] run for sending: {}", err);
            }
        });
        // run receiving task
        let receiving = tokio::spawn(async move {
            if let Err(err) = run_for_receiving(ws_reader, tx, shutdown_for_reader, meta_infos).await {
                error!("[Sender] run for receiving: {}", err);
            }
        });

        let _ = tokio::join!(sending, receiving);
        Ok(())
    }
}

/// The sending task
async fn run_for_sending(mut ws_writer: WsWriter,
                         mut rx: mpsc::Receiver<Message>,
                         mut shutdown: Shutdown) -> Result<()> {
    while !shutdown.is_shutdown() {
        let msg: Option<Message> = tokio::select! {
            res = rx.recv() => res,
            _ = shutdown.recv() => {
                // If a shutdown signal is received, return and terminate the task.
                debug!("[Sender|Handler|Writer] shutdown signal received");
                ws_writer.close().await?;
                return Ok(());
            }
        };

        if msg.is_none() {
            // channel is closed, return and terminate the task.
            debug!("[Sender|Handler|Writer] channel closed");
            ws_writer.close().await?;
            return Ok(());
        }

        if let Err(err) = ws_writer.write_message(msg.unwrap()).await {
            error!("[Sender] failed to send ws request: {}", err);
        }
    }

    Ok(())
}

/// The receiving task
async fn run_for_receiving(mut ws_reader: WsReader,
                           tx: mpsc::Sender<Message>,
                           mut shutdown: Shutdown,
                           meta_infos: BTreeMap<FileMeta, DirEntry>) -> Result<()> {
    while !shutdown.is_shutdown() {
        let msg: Message = tokio::select! {
            res = ws_reader.read_message() => res?,
            _ = shutdown.recv() => {
                // If a shutdown signal is received, return and terminate the task.
                debug!("[Sender|Handler|Reader] shutdown signal received");
                return Ok(());
            }
        };

        if msg.is_close() {
            debug!("[Sender|Handler|Reader] close message received");
            return Ok(());
        }

        // normal message, continue processing
        if let Err(err) = process_incoming_message(
            msg,
            meta_infos.clone(),
            tx.clone()).await {
            error!("[Sender] process incoming message error: {}", err);
        }
    }

    Ok(())
}

/// Process incoming message
async fn process_incoming_message(msg: Message,
                                  meta_infos: BTreeMap<FileMeta, DirEntry>,
                                  tx: mpsc::Sender<Message>) -> Result<()> {
    match msg {
        Message::Text(text) => {
            let ws_resp = serde_json::from_str::<WsResponse>(&text)?;
            match ws_resp {
                WsResponse::CreateSuccess(file_meta) => {
                    info!("[Sender] peer create file done: {:?}", file_meta);
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
                    error!("[Sender] peer create file failed: {:?}", file_meta);
                }
                WsResponse::WriteSuccess(file_meta) => {
                    info!("[Sender] peer write done: {:?}", file_meta);
                }
                WsResponse::WriteFailed(file_meta) => {
                    error!("[Sender] peer write failed: {:?}", file_meta);
                    // TODO: retry?
                }
                WsResponse::ClearDirDone(_) => {
                    info!("[Sender] peer dir is ready");
                    // spawn task to send create file message
                    meta_infos.into_iter().for_each(|(file_meta, _)| {
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            if let Err(err) = send_create_file_message(tx, file_meta).await {
                                error!("[Sender] send create file message: {}", err);
                            }
                        });
                    });
                }
            }
        },
        _ => {
            // ignore other message types
        },
    }

    Ok(())
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
