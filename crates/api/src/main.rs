use std::path::PathBuf;

use clap::Parser;
use clap_config_fallback::ConfigParser;
use eyre::Result;
use pp_api::{api::Api, config::ApiConfig};
use pp_core::config::LogConfig;

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

    // `Display` describes one layer only; eyre prints the `source()` chain once, here.
    Api::init(cli.api).await?.run().await?;

    Ok(())
}
