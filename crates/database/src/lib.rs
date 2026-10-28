//! Database access for Enoam Gallery: pool, migrations and queries.

pub mod gallery;
pub mod state;

use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};

/// Forward-only migrations embedded at compile time.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("gallery {0} not found")]
    GalleryNotFound(uuid::Uuid),
    #[error("gallery transition {from} -> {to} is not permitted")]
    InvalidTransition {
        from: state::GalleryState,
        to: state::GalleryState,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

pub async fn connect_pool(database_url: &str) -> Result<PgPool> {
    Ok(PgPoolOptions::new().connect(database_url).await?)
}

pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    Ok(MIGRATOR.run(pool).await?)
}
