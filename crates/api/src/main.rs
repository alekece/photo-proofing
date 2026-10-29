use std::{
    io::{Error as IoError, Result as IoResult},
    path::PathBuf,
};

use actix_web::{App, HttpServer, web};
use clap::Parser;
use clap_config_fallback::ConfigParser;
use pp_api::{app::configure_routes, auth::AuthState};
use pp_core::config::{AuthConfig, DatabaseConfig, LogConfig, ServerConfig, SessionConfig};
use pp_database::{MIGRATOR, PgPoolOptions, auth::AuthStore};

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

    cli.session.validate().map_err(IoError::other)?;
    cli.auth.validate().map_err(IoError::other)?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&cli.database.url)
        .await
        .map_err(|_| IoError::other("database connection failed"))?;
    MIGRATOR
        .run(&pool)
        .await
        .map_err(|_| IoError::other("database migration failed"))?;
    let state = web::Data::new(
        AuthState::new(AuthStore::new(pool), cli.session, cli.auth).map_err(IoError::other)?,
    );

    tracing::info!(host = %cli.server.host, port = cli.server.port, "api starting");

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(configure_routes)
    })
    .bind((cli.server.host.as_str(), cli.server.port))?
    .run()
    .await
}
