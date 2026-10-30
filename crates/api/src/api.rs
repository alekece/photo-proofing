use std::io::{Error as IoError, Result as IoResult};

use actix_web::{App, HttpServer, web};
use pp_database::{Database, MIGRATOR, PgPoolOptions};

use crate::{
    ApiState,
    config::{ApiConfig, HttpConfig},
    configure_routes,
};

/// The API process: [`init`](Self::init) connects and prepares everything the server needs
/// (database, migrations, authentication state), so [`run`](Self::run) only serves requests.
/// A database or configuration problem therefore stops startup instead of opening setup on an
/// unknown state.
pub struct Api {
    http: HttpConfig,
    state: web::Data<ApiState>,
}

impl Api {
    pub async fn init(config: ApiConfig) -> IoResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(config.database.url.as_str())
            .await
            .map_err(|_| IoError::other("database connection failed"))?;

        MIGRATOR
            .run(&pool)
            .await
            .map_err(|_| IoError::other("database migration failed"))?;

        let state = ApiState::new(
            Database::new(pool),
            config.session,
            config.auth,
            config.limiter,
        )
        .map_err(IoError::other)?;

        Ok(Self {
            http: config.http,
            state: web::Data::new(state),
        })
    }

    /// Serves until the process is stopped.
    pub async fn run(self) -> IoResult<()> {
        tracing::info!(host = %self.http.host, port = self.http.port, "api starting");

        let state = self.state;

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                .configure(configure_routes)
        })
        .bind((self.http.host.as_str(), self.http.port))?
        .run()
        .await
    }
}
