use std::path::Path;
use futures::future::ready;
use futures::StreamExt;
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use ws_common::{Result, AppError, Shutdown};
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

    /// Stop the websocket sender
    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // TODO: add tests
}
