use clap::Parser;
use log::{debug, error, info};
use tokio::sync::broadcast;
use ws_common::{Result, AppError};
use ws_receiver::WsReceiver;

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

// TODO: graceful shutdown
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
            // create receiver
            // let mut receiver = ws_receiver::WsReceiver::new(port, output_dir).await?;
            // if let Err(err) = receiver.run().await {
            //     error!("receiver run error: {}", err);
            //     receiver.stop().await?;
            // }
            run_receiver(port, output_dir).await
        },
        Args { port: None, output_dir: None, from: Some(from_dir), to: Some(ws_addr) } => {
            // create sender
            let sender = ws_sender::WsSender::new(from_dir, ws_addr)?;
            let stream = sender.connect().await?;
            stream.sync_dir().await?;
            Ok(())
        },
        _ => {
            Err(AppError::InvalidArgs("Invalid arguments, see --help for how to use".to_string()))
        }
    }
}

pub async fn run_receiver(port: u16, output_dir: String) -> Result<()> {
    let (shutdown_sender, _) = broadcast::channel(1);
    // create receiver
    let mut receiver = WsReceiver::new(port, output_dir, shutdown_sender).await?;

    tokio::select! {
        res = receiver.run() => {
            if let Err(err) = res {
                error!("receiver runtime error: {}", err);
            }
        },
        _ = tokio::signal::ctrl_c() => {
            info!("ctrl-c received, shut down");
        }
    }

    receiver.stop().await?;

    let WsReceiver { shutdown_sender, .. } = receiver;
    debug!("drop shutdown sender");
    drop(shutdown_sender);

    Ok(())
}
