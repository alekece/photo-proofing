use std::{io::Result as IoResult, path::PathBuf};

use actix_web::{App, HttpServer};
use clap::Parser;
use clap_config_fallback::ConfigParser;
use eg_api::app::configure_routes;
use eg_core::config::{LogConfig, ServerConfig};

#[derive(Parser, ConfigParser)]
#[command(name = "eg-api", about = "Gallery API")]
struct Cli {
    /// Optional TOML config file; see config.example.toml.
    #[arg(long, env = "EG_CONFIG")]
    #[config(path)]
    config: Option<PathBuf>,
    #[command(flatten)]
    server: ServerConfig,
    #[command(flatten)]
    log: LogConfig,
}

#[actix_web::main]
async fn main() -> IoResult<()> {
    let cli = Cli::parse_with_config();
    tracing_subscriber::fmt()
        .with_max_level(cli.log.log_level)
        .init();

    tracing::info!(host = %cli.server.host, port = cli.server.port, "api starting");

    HttpServer::new(|| App::new().configure(configure_routes))
        .bind((cli.server.host.as_str(), cli.server.port))?
        .run()
        .await
}
