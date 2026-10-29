//! PostgreSQL persistence and versioned application migrations.

pub mod auth;

pub use sqlx::{PgPool, postgres::PgPoolOptions};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
