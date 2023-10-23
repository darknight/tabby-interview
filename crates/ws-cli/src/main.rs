mod macros;

use clap::Parser;
use log::{debug, error, info};
use tokio::sync::broadcast;
use ws_common::{Result, AppError};
use ws_receiver::WsReceiver;
use ws_sender::WsSender;

/// Command line arguments for ws-cli
#[derive(Parser, Debug)]
struct Args {
    /// Required for receiver - port to listen on
    #[arg(long)]
    port: Option<u16>,
    /// Required for receiver - output directory
    #[arg(long)]
    output_dir: Option<String>,
    /// Required for sender - source directory
    #[arg(long)]
    from: Option<String>,
    /// Required for sender - websocket address of target receiver
    #[arg(long)]
    to: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // init logging
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));

    // parse command line arguments
    let args = Args::parse();
    debug!("args: {:?}", args);

    // arguments check
    match args {
        Args { port: Some(port), output_dir: Some(output_dir), from: None, to: None } => {
            ws_run!(WsReceiver, port, output_dir)
        },
        Args { port: None, output_dir: None, from: Some(from_dir), to: Some(ws_addr) } => {
            ws_run!(WsSender, from_dir, ws_addr)
        },
        _ => {
            Err(AppError::InvalidArgs("Invalid arguments, see --help for how to use".to_string()))
        }
    }
}
