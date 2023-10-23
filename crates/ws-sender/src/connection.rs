use futures::{SinkExt, StreamExt};
use futures::stream::{SplitSink, SplitStream};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, tungstenite, WebSocketStream};
use ws_common::{AppError, Result};

pub(crate) type WsSplitSink = SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>;
pub(crate) type WsSplitStream = SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>;

/// Websocket writer
pub(crate) struct WsWriter {
    outgoing: WsSplitSink,
}

/// Websocket writer implementation
impl WsWriter {

    /// Create a new websocket writer
    pub fn new(outgoing: WsSplitSink) -> Self {
        Self { outgoing }
    }

    /// Write websocket message, convert error type accordingly
    pub async fn write_message(&mut self, msg: Message) -> Result<()> {
        self.outgoing.send(msg).await.map_err(AppError::WsError)
    }

    /// Close websocket connection
    pub async fn close(&mut self) -> Result<()> {
        self.outgoing.close().await.map_err(AppError::WsError)
    }
}

/// Websocket reader
pub(crate) struct WsReader {
    incoming: WsSplitStream,
}

/// Websocket reader implementation
impl WsReader {

    /// Create a new websocket reader
    pub fn new(incoming: WsSplitStream) -> Self {
        Self { incoming }
    }

    /// Read websocket message, convert error type accordingly
    pub async fn read_message(&mut self) -> Result<Message> {
        if let Some(msg) = self.incoming.next().await {
            msg.map_err(AppError::WsError)
        } else {
            Err(AppError::WsError(tungstenite::error::Error::ConnectionClosed))
        }
    }

}
