//! PostgreSQL persistence and versioned application migrations.

pub mod auth;
pub mod gallery;

pub use sqlx::{Error as SqlError, PgPool, postgres::PgPoolOptions};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

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
