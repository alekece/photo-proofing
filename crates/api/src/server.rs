use std::io::{Error as IoError, Result as IoResult};

use actix_web::{App, HttpServer, web};
use pp_core::config::{AuthConfig, DatabaseConfig, ServerConfig, SessionConfig};
use pp_database::{Database, MIGRATOR, PgPoolOptions};

use crate::{AppState, configure_routes};

/// The API process: [`init`](Self::init) connects and prepares everything the server needs
/// (database, migrations, authentication state), so [`run`](Self::run) only serves requests.
/// A database or configuration problem therefore stops startup instead of opening setup on an
/// unknown state.
pub struct Api {
    server: ServerConfig,
    state: web::Data<AppState>,
}

impl Api {
    pub async fn init(
        server: ServerConfig,
        database: DatabaseConfig,
        session: SessionConfig,
        auth: AuthConfig,
    ) -> IoResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&database.url)
            .await
            .map_err(|_| IoError::other("database connection failed"))?;

        MIGRATOR
            .run(&pool)
            .await
            .map_err(|_| IoError::other("database migration failed"))?;

        let state = AppState::new(Database::new(pool), session, auth).map_err(IoError::other)?;

        Ok(Self {
            server,
            state: web::Data::new(state),
        })
    }

    /// Serves until the process is stopped.
    pub async fn run(self) -> IoResult<()> {
        tracing::info!(host = %self.server.host, port = self.server.port, "api starting");

        let state = self.state;

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                .configure(configure_routes)
        })
        .bind((self.server.host.as_str(), self.server.port))?
        .run()
        .await
    }
}
