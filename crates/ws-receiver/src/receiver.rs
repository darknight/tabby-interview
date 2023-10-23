use log::{debug, error, info};
use tokio::fs;
use tokio::net::TcpListener;
use ws_common::{Result, AppError, Shutdown};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, Semaphore};
use crate::{ADDR, PID_FILE};
use crate::connection::WsConnection;
use crate::fileio::{prepare_output_dir};
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
    /// The tcp listener will try to bind to `ADDR:port` and start listening.
    pub async fn new(port: u16, output_dir: String, shutdown_sender: broadcast::Sender<()>) -> Result<WsReceiver> {
        // check port
        if port < 1024 {
            return Err(AppError::SystemReservedPort(port));
        }
        // create listener
        let addr = format!("{}:{}", ADDR, port);
        let listener = TcpListener::bind(&addr).await.map_err(AppError::FailedBind)?;
        debug!("[Receiver] Listening on: {}", addr);

        prepare_output_dir(&output_dir).await?;

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
    /// keep waiting.
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
            let ws_stream = tokio_tungstenite::accept_async(stream).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    // TODO: add tests
}
