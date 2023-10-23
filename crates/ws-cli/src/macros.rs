/// Macro to run the sender or receiver
#[macro_export]
macro_rules! ws_run {
    ($which:ident, $arg1:ident, $arg2:ident) => {
        async move {
            let (shutdown_sender, _) = broadcast::channel(1);
            #[allow(unused_mut)]
            let mut instance = $which::new($arg1, $arg2, shutdown_sender).await?;

            tokio::select! {
                res = instance.run() => {
                    if let Err(err) = res {
                        error!("[{}] runtime error: {}", stringify!($which), err);
                    }
                },
                _ = tokio::signal::ctrl_c() => {
                    info!("ctrl-c received, shut down");
                }
            }

            if let Err(err) = instance.stop().await {
                error!("[{}] stop error: {}", stringify!($which), err);
            }

            let $which { shutdown_sender, .. } = instance;
            debug!("[main] drop shutdown sender");
            drop(shutdown_sender);

            Ok(())
        }.await
    };
}