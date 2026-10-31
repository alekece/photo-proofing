use std::{
    io::{Error as IoError, Result as IoResult},
    path::PathBuf,
};

use clap::Parser;
use clap_config_fallback::ConfigParser;
use pp_core::config::{LogConfig, PhotoConfig};
use pp_storage::PhotoStorage;
use tokio::signal::unix::{SignalKind, signal};

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
    #[config(no_flatten)]
    photo: PhotoConfig,
}

#[tokio::main]
async fn main() -> IoResult<()> {
    let cli = Cli::parse_with_config();
    tracing_subscriber::fmt()
        .with_max_level(cli.log.log_level)
        .init();

    // Only the startup check for now; the processing jobs will hold the handle.
    PhotoStorage::open(&cli.photo.dir)
        .await
        .map_err(|error| IoError::other(error.to_string()))?;

    tracing::info!("image worker started");

    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
    }

    tracing::info!("image worker stopping");
    Ok(())
}
