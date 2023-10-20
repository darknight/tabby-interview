use std::io::ErrorKind::WouldBlock;
use std::net::SocketAddr;
use log::{debug, error, info, warn};
use tokio::fs;
use tokio::net::{TcpListener, TcpStream};
use ws_common::{Result, AppError, WsRequest, FileEntry};
use std::path::Path;
use std::sync::{Arc, Mutex};
use futures::StreamExt;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio_tungstenite::tungstenite::Message;

const ADDR: &'static str = "0.0.0.0";
const PID_FILE: &'static str = ".receiver.pid";

#[derive(Debug)]
pub struct WsReceiver {
    output_dir: String,
    listener: TcpListener,
}

impl WsReceiver {
    /// Given `port` and `output_dir`, create a websocket receiver
    ///
    /// `port` will be checked to make sure it's valid (i.e. 1024<port<65535).
    /// `output_dir` will be checked to make sure it's valid (i.e. exists and is a directory).
    /// If `output_dir` doesn't exist, it will be created first.
    ///
    /// After the check, we'll try to bind to `ADDR:port` and start listening.
    pub async fn new(port: u16, output_dir: String) -> Result<WsReceiver> {
        // check port
        if port < 1024 || port > 65535 {
            return Err(AppError::InvalidPort(port));
        }

        // check output_dir
        let out_dir = Path::new(&output_dir);
        if out_dir.exists() {
            if !out_dir.is_dir() {
                return Err(AppError::InvalidDir(output_dir.clone()));
            }
            // check if `PID_FILE` file exists
            let pid_file = out_dir.join(PID_FILE);
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
        let peer: Arc<Mutex<Option<SocketAddr>>> = Arc::new(Mutex::new(None));
        while let Ok((stream, addr)) = self.listener.accept().await {
            info!("[Receiver] New connection from: {}", addr);
            let out = self.output_dir.clone();
            tokio::spawn(handle_connection(out, peer.clone(), stream, addr));
        }
        Ok(())
    }

    /// Stop listening for incoming connections
    ///
    /// This will also remove `PID_FILE` file
    pub async fn stop(&mut self) -> Result<()> {
        // TODO: stop listening
        // FIXME: what if fail here?
        debug!("[Receiver] Stopped listening");

        // delete PID_FILE
        let pid_file = Path::new(&self.output_dir).join(PID_FILE);
        fs::remove_file(pid_file).await.map_err(AppError::FailedDeleteFile)?;

        Ok(())
    }
}

/// Handle incoming connection
async fn handle_connection(output_dir: String, peer: Arc<Mutex<Option<SocketAddr>>>, stream: TcpStream, addr: SocketAddr) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut outgoing, mut incoming) = ws_stream.split();

    // FIXME: not compile
    // let mut lock = peer.try_lock();
    // if let Ok(mut mutex) = lock {
    //     let old = mutex.replace(addr);
    //     info!("[Receiver] drop old connection: {:?}, keep new connection: {:?}", old, addr);
    // } else {
    //     debug!("lock poisoned: {:?}", peer.is_poisoned());
    //     // TODO: differentiate `Poisoned` and `WouldBlock` error
    //     // failed to acquire lock, drop current connection
    //     warn!("[Receiver] already occupied, try to connect later");
    //     // TODO: send back error message
    //     return Ok(());
    // }

    let fut = incoming.for_each(|msg| async {
        match msg {
            Ok(msg) => {
                match msg {
                    Message::Text(text) => {
                        let ws_req = serde_json::from_str::<WsRequest>(&text);
                        if ws_req.is_err() {
                            error!("[Receiver] failed to parse message: {}", text);
                            return;
                        }
                        let ws_req = ws_req.unwrap();
                        debug!("[Receiver] received message: {:?}", ws_req);
                        match ws_req {
                            WsRequest::WriteFile(file_entry) => {
                                let res = write_file(output_dir.clone(), file_entry).await;
                            },
                            _ => {}
                        }
                    },
                    _ => {}
                }
            },
            Err(err) => {
                error!("[Receiver] read message error: {}", err);
            }
        }
    }).await;

    Ok(())
}

/// Write file to local
async fn write_file(output_dir: String, file_entry: FileEntry) -> Result<()> {
    let recv_path = Path::new(&output_dir).join(&file_entry.rel_path());
    if !recv_path.exists() {
        if file_entry.is_dir() {
            fs::create_dir_all(recv_path).await.map_err(AppError::FailedCreateDir)?;
            return Ok(());
        }

        // file_entry is file, write to local
        // file_path can not be root, so unwrap is safe
        let parent_dir = recv_path.parent().unwrap();
        if !parent_dir.exists() {
            fs::create_dir_all(parent_dir).await.map_err(AppError::FailedCreateDir)?;
        }

        // create file
        let mut file = fs::File::create(&recv_path).await.map_err(AppError::FailedCreateFile)?;

        // seek to offset, then write
        file.seek(std::io::SeekFrom::Start(file_entry.file_offset().unwrap_or(0))).await.map_err(AppError::FailedSeekFile)?;
        file.write_all(file_entry.file_content().unwrap_or(&[])).await.map_err(AppError::FailedWriteFile)?;
    }

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
}
