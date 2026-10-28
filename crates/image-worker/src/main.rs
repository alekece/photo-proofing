use std::path::PathBuf;

use clap::Parser;
use clap_config_fallback::ConfigParser;
use eg_core::config::LogConfig;
use tokio::signal::unix::{SignalKind, signal};

#[derive(Parser, ConfigParser)]
#[command(name = "eg-image-worker", about = "Enoam Gallery image worker")]
struct Cli {
    /// Optional TOML config file; see config.example.toml.
    #[arg(long, env = "EG_CONFIG")]
    #[config(path)]
    config: Option<PathBuf>,
    #[command(flatten)]
    log: LogConfig,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let cli = Cli::parse_with_config();
    tracing_subscriber::fmt()
        .with_max_level(cli.log.log_level)
        .init();

    tracing::info!("image worker started");

    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = sigterm.recv() => {}
    }

    tracing::info!("image worker stopping");
    Ok(())
}
