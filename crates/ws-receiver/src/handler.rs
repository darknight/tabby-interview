use log::{debug, error, info};
use tokio_tungstenite::tungstenite::Message;
use ws_common::{Shutdown, Result, AppError, WsRequest, WsResponse};
use crate::connection::WsConnection;
use crate::fileio::{clear_dir, create_file, write_file};

/// Handler for websocket connection
///
/// It follows `read-process-write` pattern.
/// Also, it listens for the shutdown signal and terminates the task if the signal is received.
#[derive(Debug)]
pub(crate) struct WsHandler {
    /// Output directory
    output_dir: String,
    /// Websocket connection under the hood
    ws_conn: WsConnection,
    /// Shutdown signal
    shutdown: Shutdown,
}

/// websocket handler implementation
impl WsHandler {

    /// Create a new websocket handler
    pub(crate) fn new(output_dir: String, ws_conn: WsConnection, shutdown: Shutdown) -> Self {
        Self {
            output_dir,
            ws_conn,
            shutdown,
        }
    }

    /// Run the websocket handler
    pub(crate) async fn run(&mut self) -> Result<()> {
        while !self.shutdown.is_shutdown() {
            let msg: Message = tokio::select! {
                res = self.ws_conn.read_message() => res?,
                _ = self.shutdown.recv() => {
                    // If a shutdown signal is received, return and terminate the task.
                    debug!("[Receiver|Handler] shutdown signal received");
                    return Ok(());
                }
            };

            // TODO: concurrent process
            if let Err(err) = self.process_message(msg).await {
                error!("[Receiver|Handler] process message error: {:?}", err);
                continue;
            }
        }

        Ok(())
    }

    /// Process websocket message
    async fn process_message(&mut self, msg: Message) -> Result<()> {
        match msg {
            Message::Text(text) => {
                let ws_req = serde_json::from_str::<WsRequest>(&text)?;
                match ws_req {
                    WsRequest::CreateFile(file_meta) => {
                        debug!("[Receiver] got create file message: {:?}", file_meta);
                        let out = self.output_dir.clone();
                        let resp = match create_file(out, file_meta.clone()).await {
                            Ok(_) => {
                                WsResponse::new_create_success_message(file_meta)?
                            }
                            Err(err) => {
                                error!("[Receiver] create file error: {:?}", err);
                                WsResponse::new_create_failed_message(file_meta)?
                            }
                        };
                        self.ws_conn.write_message(resp).await?;
                    }
                    WsRequest::WriteFile(file_entry) => {
                        debug!("[Receiver] got write file message: {:?}", file_entry);
                        let out = self.output_dir.clone();
                        let file_meta = file_entry.file_meta();
                        let resp = match write_file(out, file_entry).await {
                            Ok(_) => {
                                WsResponse::new_write_success_message(file_meta)?
                            }
                            Err(err) => {
                                error!("[Receiver] write file error: {:?}", err);
                                WsResponse::new_write_failed_message(file_meta)?
                            }
                        };
                        self.ws_conn.write_message(resp).await?;
                    }
                    WsRequest::ClearDir(_) => {
                        info!("[Receiver] got clear dir message");
                        let out = self.output_dir.clone();
                        if let Err(err) = clear_dir(out).await {
                            error!("[Receiver] clear dir error: {:?}", err);
                            // TODO: retry?
                        }
                        let resp = WsResponse::new_clear_dir_done_message(vec![])?;
                        self.ws_conn.write_message(resp).await?;
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}
