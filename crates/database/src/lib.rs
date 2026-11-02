//! PostgreSQL persistence and versioned application migrations.

pub mod auth;
pub mod error;
pub mod gallery;
pub mod jobs;
pub mod photo;
pub mod photo_intake;

pub use sqlx::PgPool;
use sqlx::{Connection, PgConnection, postgres::PgPoolOptions};

use crate::error::DatabaseError;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// A data-access object for one kind of entity, borrowed from a [`Database`].
///
/// `Options` carries settings the store needs beyond the pool; stores without any use `()`.
pub trait Repository<'db> {
    type Options;

    fn open(pool: &'db PgPool, options: Self::Options) -> Self;
}

/// The application's PostgreSQL connection pool. Entities are reached through
/// [`repository`](Self::repository) instead of one method per entity, so adding an entity
/// means adding a [`Repository`] implementation and leaves this type unchanged.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Parses `url` and builds a pool that opens connections on demand, so it does no I/O.
    /// Call [`ensure_reachable`](Self::ensure_reachable) to find out whether the server answers.
    pub fn connect_lazy(url: impl AsRef<str>, max_connections: u32) -> Result<Self, DatabaseError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect_lazy(url.as_ref())?;

        Ok(Self::new(pool))
    }

    /// Makes exactly one standalone connection attempt and closes it again.
    ///
    /// A pool retries a failing connection until its acquire timeout and then reports only
    /// "timed out", which hides why the server is unreachable. A single attempt reports the
    /// real cause immediately.
    pub async fn ensure_reachable(&self) -> Result<(), DatabaseError> {
        let connection = PgConnection::connect_with(&self.pool.connect_options()).await?;

        Ok(connection.close().await?)
    }

    /// Applies the pending migrations.
    pub async fn migrate(&self) -> Result<(), DatabaseError> {
        Ok(MIGRATOR.run(&self.pool).await?)
    }

    /// Opens the repository `R` with its default options.
    pub fn repository<'db, R>(&'db self) -> R
    where
        R: Repository<'db>,
        R::Options: Default,
    {
        self.repository_with(R::Options::default())
    }

    /// Opens the repository `R` with explicit options.
    pub fn repository_with<'db, R: Repository<'db>>(&'db self, options: R::Options) -> R {
        R::open(&self.pool, options)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    #[tokio::test]
    async fn ensure_reachable_reports_a_refused_connection_immediately() {
        let database = Database::connect_lazy("postgres://user:s3cretpw@127.0.0.1:1/pp", 1)
            .expect("the URL is well formed");

        let error = database.ensure_reachable().await.unwrap_err();

        assert!(matches!(error, DatabaseError::Unreachable(_)), "{error}");
        assert!(!error.to_string().contains("s3cretpw"));
        assert!(
            error
                .source()
                .is_some_and(|cause| !cause.to_string().contains("s3cretpw"))
        );
    }

    #[tokio::test]
    async fn malformed_urls_are_configuration_errors_without_detail() {
        let result = Database::connect_lazy("postgres://u:s3cretpw@h/pp?sslmode=s3cretpw", 1);

        let Err(error) = result else {
            panic!("an unknown sslmode must be rejected");
        };

        assert!(matches!(error, DatabaseError::Configuration));
        assert!(!error.to_string().contains("s3cretpw"));
    }
}
