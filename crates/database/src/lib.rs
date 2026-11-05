//! PostgreSQL persistence and versioned application migrations.

pub mod auth;
pub mod error;
pub mod favorite_events;
pub mod gallery;
pub mod jobs;
pub mod listener;
pub mod photo;
pub mod photo_events;
pub mod photo_intake;
pub mod photo_retry;
#[cfg(any(test, feature = "tests"))]
pub mod tests;

pub use sqlx::PgPool;
use sqlx::{Connection, PgConnection, postgres::PgPoolOptions};

use crate::{
    error::DatabaseError,
    listener::{Channel, Listener},
};

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

    /// Opens a [`Listener`] on `T`'s channel. Only notifications committed after this returns
    /// are reported.
    pub async fn listen<T: Channel>(&self) -> Result<Listener<T>, DatabaseError> {
        Listener::open(&self.pool).await
    }
}
