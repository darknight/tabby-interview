use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, tungstenite, WebSocketStream};
use tokio_tungstenite::tungstenite::Message;
use ws_common::{AppError, Result};

/// Websocket connection
#[derive(Debug)]
pub(crate) struct WsConnection {
    ws_stream: WebSocketStream<TcpStream>,
}

/// Websocket connection implementation
impl WsConnection {

    /// Create a new websocket connection
    pub fn new(ws_stream: WebSocketStream<TcpStream>) -> Self {
        Self { ws_stream }
    }

    /// Read websocket message, convert error type accordingly
    pub async fn read_message(&mut self) -> Result<Message> {
        if let Some(msg) = self.ws_stream.next().await {
            msg.map_err(AppError::WsError)
        } else {
            Err(AppError::WsError(tungstenite::error::Error::ConnectionClosed))
        }
    }

    /// Write websocket message, convert error type accordingly
    pub async fn write_message(&mut self, msg: Message) -> Result<()> {
        self.ws_stream.send(msg).await.map_err(AppError::WsError)
    }
}
