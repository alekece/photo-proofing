use std::{io::Result as IoResult, path::PathBuf};

use clap::Parser;
use clap_config_fallback::ConfigParser;
use pp_api::server::Api;
use pp_core::config::{AuthConfig, DatabaseConfig, LogConfig, ServerConfig, SessionConfig};

#[derive(Parser, ConfigParser)]
#[command(name = "pp-api", about = "Gallery API")]
struct Cli {
    /// Optional TOML config file; see config.example.toml.
    #[arg(long, env = "PP_CONFIG")]
    #[config(path)]
    config: Option<PathBuf>,
    #[command(flatten)]
    server: ServerConfig,
    #[command(flatten)]
    log: LogConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    session: SessionConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    database: DatabaseConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    auth: AuthConfig,
}

#[actix_web::main]
async fn main() -> IoResult<()> {
    let cli = Cli::parse_with_config();

    tracing_subscriber::fmt()
        .with_max_level(cli.log.log_level)
        .init();

    Api::init(cli.server, cli.database, cli.session, cli.auth)
        .await?
        .run()
        .await
}
