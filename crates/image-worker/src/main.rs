use std::{
    io::{Error as IoError, Result as IoResult},
    path::PathBuf,
};

use clap::Parser;
use clap_config_fallback::ConfigParser;
use pp_core::config::LogConfig;
use pp_image_worker::{config::WorkerConfig, handlers::HandlerRegistry, runtime::Worker};
use tokio::{
    signal::{
        self,
        unix::{SignalKind, signal},
    },
    task,
};
use tokio_util::sync::CancellationToken;

#[derive(Parser, ConfigParser)]
#[command(name = "pp-image-worker", about = "Gallery image worker")]
struct Cli {
    /// Optional TOML config file; see config.example.toml.
    #[arg(long, env = "PP_CONFIG")]
    #[config(path)]
    config: Option<PathBuf>,
    #[command(flatten)]
    log: LogConfig,
    #[command(flatten)]
    worker: WorkerConfig,
}

#[tokio::main]
async fn main() -> IoResult<()> {
    let cli = Cli::parse_with_config();
    tracing_subscriber::fmt()
        .with_max_level(cli.log.log_level)
        .init();

    tracing::info!("image worker started");

    // Listening starts before the database is reached so a stop signal during startup is
    // not lost.
    let mut sigterm = signal(SignalKind::terminate())?;
    let shutdown = CancellationToken::new();
    let signals = task::spawn({
        let shutdown = shutdown.clone();
        async move {
            tokio::select! {
                _ = signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
            shutdown.cancel();
        }
    });

    // Photo processing registers its handler here in a later slice.
    let worker = Worker::connect(cli.worker, HandlerRegistry::default())
        .await
        .map_err(|error| IoError::other(error.to_string()))?;
    worker.run(shutdown).await;
    signals.abort();

    tracing::info!("image worker stopping");
    Ok(())
}
