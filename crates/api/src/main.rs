use std::path::PathBuf;

use clap::Parser;
use clap_config_fallback::ConfigParser;
use eyre::Result;
use pp_api::{api::Api, config::ApiConfig};
use pp_core::config::LogConfig;
use tokio::signal::{
    self,
    unix::{SignalKind, signal},
};
use tokio_util::sync::CancellationToken;

#[derive(Parser, ConfigParser)]
#[command(name = "pp-api", about = "Gallery API")]
struct Cli {
    /// Optional TOML config file; see config.example.toml.
    #[arg(long, env = "PP_CONFIG")]
    #[config(path)]
    config: Option<PathBuf>,
    #[command(flatten)]
    log: LogConfig,
    #[command(flatten)]
    api: ApiConfig,
}

#[actix_web::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse_with_config();

    tracing_subscriber::fmt()
        .with_max_level(cli.log.log_level)
        .init();

    // Listening starts before the database is reached so a stop signal during startup is
    // not lost.
    let mut sigterm = signal(SignalKind::terminate())?;
    let shutdown = CancellationToken::new();
    let signals = tokio::spawn({
        let shutdown = shutdown.clone();

        async move {
            tokio::select! {
                _ = signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }

            shutdown.cancel();
        }
    });
    // `Display` describes one layer only; eyre prints the `source()` chain once, here.
    let result = async { Api::init(cli.api).await?.run(shutdown).await }.await;

    signals.abort();

    Ok(result?)
}
