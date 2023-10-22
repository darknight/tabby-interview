use std::net::SocketAddr;
use log::{debug, error, info, warn};
use tokio::fs;
use tokio::net::{TcpListener, TcpStream};
use ws_common::{Result, AppError, WsRequest, FileEntry, WsResponse, FileMeta, Shutdown};
use std::path::Path;
use std::sync::{Arc, Mutex};
use futures::{SinkExt, StreamExt};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, OwnedSemaphorePermit, Semaphore};
use tokio_tungstenite::tungstenite::Message;
use crate::{ADDR, CHANNEL_CAPACITY, PID_FILE};
use crate::connection::WsConnection;
use crate::fileio::{clear_dir, create_file, write_file};
use crate::handler::WsHandler;

/// Websocket receiver
#[derive(Debug)]
pub struct WsReceiver {
    output_dir: String,
    listener: TcpListener,
    /// Limit the max number of connections
    /// For our use case, we only need to accept one connection at a time
    only_one_sender: Arc<Semaphore>,
    /// Broadcast shutdown signal to all active connections
    pub shutdown_sender: broadcast::Sender<()>,
}

impl WsReceiver {
    /// Given `port` and `output_dir`, create a websocket receiver
    ///
    /// `port` will be checked to make sure it's valid (i.e. 1024<port<65535).
    /// `output_dir` will be checked to make sure it's valid (i.e. exists and is a directory).
    /// If `output_dir` doesn't exist, it will be created first.
    ///
    /// After the check, we'll try to bind to `ADDR:port` and start listening.
    pub async fn new(port: u16, output_dir: String, shutdown_sender: broadcast::Sender<()>) -> Result<WsReceiver> {
        // check port
        if port < 1024 {
            return Err(AppError::SystemReservedPort(port));
        }

        // check output_dir
        let out_dir = Path::new(&output_dir);
        if out_dir.exists() {
            if !out_dir.is_dir() {
                return Err(AppError::InvalidDir(output_dir.clone()));
            }
            // check if `PID_FILE` file exists
            let pid_file = out_dir.join(PID_FILE);
            warn!("pid file: {:?}, exists? {}", pid_file, pid_file.exists());
            if pid_file.exists() {
                return Err(AppError::DirInUse(output_dir.clone()));
            }
        } else {
            // create output_dir
            fs::create_dir_all(out_dir).await.map_err(AppError::FailedCreateDir)?;
        }

        // start receiver server
        let addr = format!("{}:{}", ADDR, port);
        let listener = TcpListener::bind(&addr).await.map_err(AppError::FailedBind)?;
        debug!("[Receiver] Listening on: {}", addr);

        // create `PID_FILE` for current receiver
        let pid_file = out_dir.join(PID_FILE);
        fs::write(pid_file, format!("{}", std::process::id())).await
            .map_err(AppError::FailedWriteFile)?;
        debug!("[Receiver] Done writing pid file in dir: {:?}", out_dir);

        // all good, return WsReceiver instance
        Ok(WsReceiver {
            output_dir,
            listener,
            only_one_sender: Arc::new(Semaphore::new(1)),
            shutdown_sender,
        })
    }

    /// Start accepting incoming connections
    ///
    /// Current design is to only accept one connection at a time. All subsequent connections will
    /// receive an error message and be rejected.
    ///
    /// NOTE:
    /// Alternatively, we can use a queue to hold all incoming connections, but this will
    /// waste receiver resources and gain nothing, since syncing directory is more like 1:1 mapping
    pub async fn run(&mut self) -> Result<()> {
        info!("[Receiver] Listening for incoming connections");

        loop {
            let permit = self.only_one_sender.clone().acquire_owned()
                .await.map_err(AppError::SemaphoreAcquireError)?;
            let (stream, addr) = self.listener.accept().await.map_err(AppError::SocketError)?;
            info!("[Receiver] New connection from: {}", addr);

            // create ws stream based on tcp stream
            let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;
            let mut ws_handler = WsHandler::new(
                self.output_dir.clone(),
                WsConnection::new(ws_stream),
                Shutdown::new(self.shutdown_sender.subscribe()));

            tokio::spawn(async move {
                if let Err(err) = ws_handler.run().await {
                    error!("[Receiver] ws handler error: {}", err);
                }
                drop(permit);
            });
        }
    }

    /// Clean up receiver
    ///
    /// This will remove `PID_FILE` file
    pub async fn stop(&mut self) -> Result<()> {
        debug!("[Receiver] cleanup...");
        // close semaphore
        self.only_one_sender.close();

        // delete PID_FILE
        let pid_file = Path::new(&self.output_dir).join(PID_FILE);
        fs::remove_file(pid_file).await.map_err(AppError::FailedDeleteFile)?;

        Ok(())
    }
}

// /// Handle incoming connection
// async fn handle_connection(output_dir: String, stream: TcpStream, permit: Option<OwnedSemaphorePermit>) -> Result<()> {
//     let mut ws_stream = tokio_tungstenite::accept_async(stream).await?;
//
//     // send receiver busy message and close stream
//     if permit.is_none() {
//         let resp = WsResponse::new_receiver_busy_message()?;
//         ws_stream.send(resp).await?;
//         return Ok(());
//     }
//
//     let (mut outgoing, mut incoming) = ws_stream.split();
//     // create channel to collect file entry from tasks
//     let (tx, mut rx) = mpsc::channel::<Message>(CHANNEL_CAPACITY);
//
//     // spawn a task to collect message and send back to `sender`
//     tokio::spawn(async move {
//         while let Some(msg) = rx.recv().await {
//             debug!("[Receiver] send ws response: {:?}", msg);
//             if let Err(err) = outgoing.send(msg).await {
//                 error!("[Receiver] failed to send ws response: {}", err);
//             }
//         }
//     });
//
//     while let Some(msg) = incoming.next().await {
//         match msg {
//             Ok(msg) => {
//                 match msg {
//                     Message::Text(text) => {
//                         let ws_req = serde_json::from_str::<WsRequest>(&text);
//                         if ws_req.is_err() {
//                             error!("[Receiver] failed to parse message: {}", text);
//                             continue;
//                         }
//                         let ws_req = ws_req.unwrap();
//                         match ws_req {
//                             WsRequest::CreateFile(file_meta) => {
//                                 debug!("[Receiver] got create file message: {:?}", file_meta);
//                                 let tx = tx.clone();
//                                 let out = output_dir.clone();
//                                 tokio::spawn(async move {
//                                     let resp = match create_file(out, file_meta.clone()).await {
//                                         Ok(_) => {
//                                             WsResponse::new_create_success_message(file_meta)
//                                         }
//                                         Err(err) => {
//                                             error!("[Receiver] create file error: {:?}", err);
//                                             WsResponse::new_create_failed_message(file_meta)
//                                         }
//                                     };
//                                     if let Err(err) = resp {
//                                         error!("[Receiver] failed to create ws response: {:?}", err);
//                                         return;
//                                     }
//                                     if let Err(err) = tx.send(resp.unwrap()).await {
//                                         error!("[Receiver] failed to send ws response: {}", err);
//                                     }
//                                 });
//                             }
//                             WsRequest::WriteFile(file_entry) => {
//                                 debug!("[Receiver] got write file message: {:?}", file_entry);
//                                 let tx = tx.clone();
//                                 let out = output_dir.clone();
//                                 tokio::spawn(async move {
//                                     let file_meta = file_entry.file_meta();
//                                     let resp = match write_file(out, file_entry).await {
//                                         Ok(_) => {
//                                             WsResponse::new_write_success_message(file_meta)
//                                         }
//                                         Err(err) => {
//                                             error!("[Receiver] write file error: {:?}", err);
//                                             WsResponse::new_write_failed_message(file_meta)
//                                         }
//                                     };
//                                     if let Err(err) = resp {
//                                         error!("[Receiver] failed to create ws response: {:?}", err);
//                                         return;
//                                     }
//                                     if let Err(err) = tx.send(resp.unwrap()).await {
//                                         error!("[Receiver] failed to send ws response: {}", err);
//                                     }
//                                 });
//                             }
//                             WsRequest::ClearDir(_) => {
//                                 info!("[Receiver] got clear dir message");
//                                 // accept new connection, clear local dir, send response
//                                 if let Err(err) = clear_dir(output_dir.clone()).await {
//                                     error!("[Receiver] failed to clear dir: {:?}", err);
//                                     // TODO: retry?
//                                 }
//                                 let resp = WsResponse::new_clear_dir_done_message(vec![]);
//                                 if let Err(err) = resp {
//                                     error!("[Receiver] failed to create ws response: {:?}", err);
//                                     continue;
//                                 }
//                                 if let Err(err) = tx.send(resp.unwrap()).await {
//                                     error!("[Receiver] failed to send ws response: {}", err);
//                                 }
//                             }
//                         }
//                     }
//                     _ => {}
//                 }
//             }
//             Err(err) => {
//                 error!("[Receiver] read message error: {}", err);
//             }
//         }
//     }
//
//     Ok(())
// }

#[cfg(test)]
mod tests {
    use super::*;
}
