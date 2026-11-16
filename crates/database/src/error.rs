//! The only error type `pp-database` exposes for storage failures.
//!
//! [`DatabaseError::from`] is the single place that classifies `sqlx` errors. Its invariant is
//! that nothing in the result can contain the database URL or password: configuration errors
//! can echo URL components, so they are reduced to a fixed message, and every other kind is
//! built only from connection-phase or server output.

use std::io;

use sqlx::migrate::MigrateError;
use thiserror::Error;

/// SQLSTATE class for invalid authorization specifications (`28xxx`).
const AUTHORIZATION_CLASS: &str = "28";

/// SQLSTATE for unique-constraint violations (`23505`).
const UNIQUE_VIOLATION: &str = "23505";

/// PostgreSQL `UNIQUE` constraint violations (`23505`) classified by [`DatabaseError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueViolation {
    /// `galleries_share_token_unique` (`0003_share_tokens.sql`).
    GalleryShareToken,
    /// `contacts_workspace_email_unique` (`0031_contacts.sql`).
    ContactWorkspaceEmail,
    /// `gallery_contacts_share_token_unique` (`0031_contacts.sql`).
    GalleryContactShareToken,
    /// Any other unique-constraint violation not yet explicitly modeled.
    Unknown,
}

impl UniqueViolation {
    fn from_constraint(constraint: Option<&str>) -> Self {
        match constraint {
            Some("galleries_share_token_unique") => Self::GalleryShareToken,
            Some("contacts_workspace_email_unique") => Self::ContactWorkspaceEmail,
            Some("gallery_contacts_share_token_unique") => Self::GalleryContactShareToken,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database unreachable")]
    Unreachable(#[source] io::Error),
    #[error("database authentication failed: {0}")]
    Authentication(String),
    #[error("database TLS failure: {0}")]
    Tls(String),
    /// Deliberately without detail: `sqlx` configuration errors can echo parts of the URL.
    #[error("database configuration is invalid")]
    Configuration,
    #[error("timed out waiting for a database connection")]
    Timeout,
    #[error("database migration failed: {0}")]
    Migration(String),
    #[error("database rejected the request: {message}")]
    UniqueViolation {
        violation: UniqueViolation,
        message: String,
    },
    #[error("database rejected the request: {message}")]
    Server {
        code: Option<String>,
        message: String,
    },
    #[error("database error: {0}")]
    Other(String),
}

impl DatabaseError {
    /// Whether this error is a PostgreSQL unique-constraint violation (`23505`) matching
    /// `expected`.
    pub fn is_unique_violation(&self, expected: UniqueViolation) -> bool {
        !matches!(expected, UniqueViolation::Unknown)
            && matches!(
                self,
                Self::UniqueViolation { violation, .. } if *violation == expected
            )
    }
}

impl From<sqlx::Error> for DatabaseError {
    fn from(error: sqlx::Error) -> Self {
        match error {
            sqlx::Error::Io(error) => Self::Unreachable(error),
            sqlx::Error::Tls(error) => Self::Tls(error.to_string()),
            sqlx::Error::Configuration(_) => Self::Configuration,
            sqlx::Error::PoolTimedOut => Self::Timeout,
            sqlx::Error::Migrate(error) => Self::from(*error),
            sqlx::Error::Database(error) => {
                let code = error.code().map(String::from);
                let message = error.message().to_owned();

                // The detail can hold row values, so it is only shown in debug builds.
                #[cfg(debug_assertions)]
                let message = match error
                    .try_downcast_ref::<sqlx::postgres::PgDatabaseError>()
                    .and_then(sqlx::postgres::PgDatabaseError::detail)
                {
                    Some(detail) => format!("{message} ({detail})"),
                    None => message,
                };

                match code {
                    Some(code) if code == UNIQUE_VIOLATION => Self::UniqueViolation {
                        violation: UniqueViolation::from_constraint(error.constraint()),
                        message,
                    },
                    Some(code) if code.starts_with(AUTHORIZATION_CLASS) => {
                        Self::Authentication(message)
                    }
                    code => Self::Server { code, message },
                }
            }
            error => Self::Other(error.to_string()),
        }
    }
}

impl From<MigrateError> for DatabaseError {
    fn from(error: MigrateError) -> Self {
        match error {
            MigrateError::Execute(error) => Self::from(error),
            error => Self::Migration(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use sqlx::PgPool;

    use super::*;

    #[test]
    fn configuration_errors_never_echo_the_url() {
        let error = DatabaseError::from(sqlx::Error::Configuration(
            "unknown value \"s3cretpw\" for `ssl_mode`".into(),
        ));

        assert!(!error.to_string().contains("s3cretpw"));
        assert!(error.source().is_none());
    }

    #[test]
    fn io_errors_are_unreachable_with_their_cause() {
        let error = DatabaseError::from(sqlx::Error::Io(io::Error::from(
            io::ErrorKind::ConnectionRefused,
        )));

        assert!(matches!(error, DatabaseError::Unreachable(_)));
        assert!(error.source().is_some());
    }

    #[test]
    fn pool_timeouts_map_to_timeout() {
        assert!(matches!(
            DatabaseError::from(sqlx::Error::PoolTimedOut),
            DatabaseError::Timeout
        ));
    }

    #[sqlx::test]
    async fn server_errors_keep_code_and_message(pool: PgPool) {
        let error = sqlx::query("SELECT 1/0").execute(&pool).await.unwrap_err();

        let DatabaseError::Server { code, message } = DatabaseError::from(error) else {
            panic!("expected a server error");
        };

        assert_eq!(code.as_deref(), Some("22012"));
        assert!(message.contains("division by zero"));
    }

    #[sqlx::test]
    async fn unmapped_unique_violation_maps_to_unknown(pool: PgPool) {
        sqlx::query(
            "CREATE TABLE unique_probe (
                 email TEXT NOT NULL CONSTRAINT unique_probe_email_key UNIQUE
             )",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO unique_probe(email) VALUES ('a@example.test')")
            .execute(&pool)
            .await
            .unwrap();

        let dup = sqlx::query("INSERT INTO unique_probe(email) VALUES ('a@example.test')")
            .execute(&pool)
            .await
            .unwrap_err();
        let error = DatabaseError::from(dup);

        assert!(matches!(
            error,
            DatabaseError::UniqueViolation {
                violation: UniqueViolation::Unknown,
                ..
            }
        ));
        assert!(!error.is_unique_violation(UniqueViolation::Unknown));
        assert!(!error.is_unique_violation(UniqueViolation::ContactWorkspaceEmail));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn unique_violation_variants_match_migrated_constraints(pool: PgPool) {
        let constraints: Vec<String> =
            sqlx::query_scalar("SELECT conname FROM pg_constraint WHERE contype = 'u'")
                .fetch_all(&pool)
                .await
                .unwrap();

        for (name, expected) in [
            (
                "galleries_share_token_unique",
                UniqueViolation::GalleryShareToken,
            ),
            (
                "contacts_workspace_email_unique",
                UniqueViolation::ContactWorkspaceEmail,
            ),
            (
                "gallery_contacts_share_token_unique",
                UniqueViolation::GalleryContactShareToken,
            ),
        ] {
            assert!(
                constraints.iter().any(|conname| conname == name),
                "missing migrated UNIQUE constraint {name}"
            );
            assert_eq!(UniqueViolation::from_constraint(Some(name)), expected);
        }
    }
}
