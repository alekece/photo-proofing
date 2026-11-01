//! Persistence for the single account and its revocable sessions.
//!
//! A session is a row keyed by the SHA-256 digest of the cookie token, with an absolute
//! `expires_at`. Revoking deletes the row, so copied tokens stop working immediately.
//! Expiry is sliding: renewal pushes `expires_at` forward, with no maximum age.

use std::time::SystemTime;

use pp_core::email::Email;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Type};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Repository, error::DatabaseError};

/// Identifies the single photographer account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct AccountId(pub Uuid);

/// Owner of galleries and settings. Kept separate from [`AccountId`] so collaborators can
/// later join a workspace without changing who owns its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct WorkspaceId(pub Uuid);

/// Who a valid session belongs to; the only thing protected operations should trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct Identity {
    pub account_id: AccountId,
    pub workspace_id: WorkspaceId,
}

/// Data access for the account and its sessions; see the module docs for the session model.
#[derive(Clone, Copy)]
pub struct AuthRepository<'db> {
    pool: &'db PgPool,
}

#[derive(FromRow)]
pub struct Account {
    pub id: AccountId,
    pub workspace_id: WorkspaceId,
    pub password_hash: String,
}

impl Account {
    pub fn identity(&self) -> Identity {
        Identity {
            account_id: self.id,
            workspace_id: self.workspace_id,
        }
    }
}

impl<'db> Repository<'db> for AuthRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl AuthRepository<'_> {
    /// True while no account exists, i.e. while the login page should offer setup.
    pub async fn setup_available(&self) -> Result<bool, DatabaseError> {
        Ok(
            sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM accounts)")
                .fetch_one(self.pool)
                .await?,
        )
    }

    /// Creates the workspace, account and first session in one transaction. Returns `None`
    /// when an account already exists; the `singleton` unique column makes concurrent
    /// setups safe, and a conflict rolls back the newly inserted workspace as well.
    pub async fn create_account(
        &self,
        email: &Email,
        password_hash: &str,
        token_hash: &[u8; 32],
        expires_at: SystemTime,
    ) -> Result<Option<Identity>, DatabaseError> {
        let mut tx = self.pool.begin().await?;
        let workspace_id = WorkspaceId(Uuid::new_v4());
        let account_id = AccountId(Uuid::new_v4());

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;

        let inserted = sqlx::query("INSERT INTO accounts(id,workspace_id,email,password_hash) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
            .bind(account_id).bind(workspace_id).bind(email.as_str()).bind(password_hash).execute(&mut *tx).await?.rows_affected();

        if inserted == 0 {
            tx.rollback().await?;

            return Ok(None);
        }

        sqlx::query("INSERT INTO sessions(token_hash,account_id,expires_at) VALUES ($1,$2,$3)")
            .bind(token_hash.as_slice())
            .bind(account_id)
            .bind(OffsetDateTime::from(expires_at))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(Some(Identity {
            account_id,
            workspace_id,
        }))
    }

    pub async fn account_by_email(&self, email: &Email) -> Result<Option<Account>, DatabaseError> {
        Ok(
            sqlx::query_as("SELECT id,workspace_id,password_hash FROM accounts WHERE email=$1")
                .bind(email.as_str())
                .fetch_optional(self.pool)
                .await?,
        )
    }

    /// Inserts a session, first deleting `replaced_hash` (the caller's previous session on
    /// the same account) so repeated logins from one browser do not accumulate rows.
    /// Other devices' sessions are untouched.
    pub async fn create_session(
        &self,
        account_id: AccountId,
        token_hash: &[u8; 32],
        expires_at: SystemTime,
        replaced_hash: Option<&[u8; 32]>,
    ) -> Result<(), DatabaseError> {
        let mut tx = self.pool.begin().await?;

        if let Some(hash) = replaced_hash {
            sqlx::query("DELETE FROM sessions WHERE token_hash=$1 AND account_id=$2")
                .bind(hash.as_slice())
                .bind(account_id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query("INSERT INTO sessions(token_hash,account_id,expires_at) VALUES ($1,$2,$3)")
            .bind(token_hash.as_slice())
            .bind(account_id)
            .bind(OffsetDateTime::from(expires_at))
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(())
    }

    /// Read-only: identifies the session owner if it has not expired. Never extends expiry.
    pub async fn authenticate(
        &self,
        token_hash: &[u8; 32],
        now: SystemTime,
    ) -> Result<Option<Identity>, DatabaseError> {
        Ok(sqlx::query_as("SELECT a.id AS account_id,a.workspace_id FROM sessions s JOIN accounts a ON a.id=s.account_id WHERE s.token_hash=$1 AND s.expires_at>$2")
            .bind(token_hash.as_slice()).bind(OffsetDateTime::from(now)).fetch_optional(self.pool).await?)
    }

    /// Extends a live session and returns its owner, or `None` if it is expired or revoked.
    /// `GREATEST` means a late or out-of-order renewal can never shorten the expiry.
    pub async fn renew(
        &self,
        token_hash: &[u8; 32],
        now: SystemTime,
        expires_at: SystemTime,
    ) -> Result<Option<Identity>, DatabaseError> {
        // UPDATE, never UPSERT: a revoked session must not be recreated by renewal.
        Ok(sqlx::query_as("UPDATE sessions s SET expires_at=GREATEST(s.expires_at,$3) FROM accounts a WHERE s.account_id=a.id AND s.token_hash=$1 AND s.expires_at>$2 RETURNING a.id AS account_id,a.workspace_id")
            .bind(token_hash.as_slice()).bind(OffsetDateTime::from(now)).bind(OffsetDateTime::from(expires_at)).fetch_optional(self.pool).await?)
    }

    /// Deletes the session; copies of its cookie stop authenticating at once. Idempotent.
    pub async fn revoke(&self, token_hash: &[u8; 32]) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(token_hash.as_slice())
            .execute(self.pool)
            .await?;

        Ok(())
    }

    /// Housekeeping only: expired sessions already fail `authenticate`, so deleting at most
    /// `limit` per call just bounds table growth without long locks.
    pub async fn cleanup_expired(&self, now: SystemTime, limit: i64) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE expires_at<=$1 ORDER BY expires_at LIMIT $2)").bind(OffsetDateTime::from(now)).bind(limit).execute(self.pool).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::Database;

    fn days(count: u64) -> Duration {
        Duration::from_secs(count * 24 * 60 * 60)
    }

    fn email(value: &str) -> Email {
        value.parse().unwrap()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn setup_session_and_revocation_are_persistent(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<AuthRepository>();
        let now = SystemTime::now();

        assert!(store.setup_available().await.unwrap());

        let identity = store
            .create_account(
                &email("person@example.test"),
                "hash",
                &[1; 32],
                now + days(30),
            )
            .await
            .unwrap()
            .unwrap();

        assert!(!store.setup_available().await.unwrap());
        assert!(
            store
                .create_account(
                    &email("other@example.test"),
                    "hash",
                    &[2; 32],
                    now + days(30)
                )
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store.authenticate(&[1; 32], now).await.unwrap(),
            Some(identity)
        );
        assert!(
            store
                .authenticate(&[1; 32], now + days(30))
                .await
                .unwrap()
                .is_none()
        );

        store
            .renew(&[1; 32], now, now + days(40))
            .await
            .unwrap()
            .unwrap();

        assert!(
            store
                .authenticate(&[1; 32], now + days(31))
                .await
                .unwrap()
                .is_some()
        );

        store.revoke(&[1; 32]).await.unwrap();

        assert!(
            store
                .renew(&[1; 32], now, now + days(50))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            Database::new(pool)
                .repository::<AuthRepository>()
                .authenticate(&[1; 32], now)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn concurrent_setup_leaves_one_workspace_and_account(pool: PgPool) {
        let database = Database::new(pool.clone());
        let a = database.repository::<AuthRepository>();
        let b = a;
        let expiry = SystemTime::now() + days(30);
        let (email_a, email_b) = (email("a@example.test"), email("b@example.test"));
        let (a, b) = tokio::join!(
            a.create_account(&email_a, "hash", &[1; 32], expiry),
            b.create_account(&email_b, "hash", &[2; 32], expiry)
        );

        assert_eq!(
            usize::from(a.unwrap().is_some()) + usize::from(b.unwrap().is_some()),
            1
        );

        for table in ["accounts", "workspaces", "sessions"] {
            let count: i64 =
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                    .fetch_one(&pool)
                    .await
                    .unwrap();

            assert_eq!(count, 1);
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn renewal_and_logout_cannot_resurrect_session(pool: PgPool) {
        let database = Database::new(pool);
        let store = database.repository::<AuthRepository>();
        let now = SystemTime::now();

        store
            .create_account(&email("a@example.test"), "hash", &[1; 32], now + days(1))
            .await
            .unwrap();

        let (renew, revoke) = tokio::join!(
            store.renew(&[1; 32], now, now + days(2)),
            store.revoke(&[1; 32])
        );

        renew.unwrap();
        revoke.unwrap();

        assert!(store.authenticate(&[1; 32], now).await.unwrap().is_none());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn replacing_one_login_does_not_revoke_another(pool: PgPool) {
        let database = Database::new(pool);
        let store = database.repository::<AuthRepository>();
        let now = SystemTime::now();
        let id = store
            .create_account(&email("a@example.test"), "hash", &[1; 32], now + days(1))
            .await
            .unwrap()
            .unwrap();

        store
            .create_session(id.account_id, &[2; 32], now + days(1), None)
            .await
            .unwrap();
        store
            .create_session(id.account_id, &[3; 32], now + days(1), Some(&[1; 32]))
            .await
            .unwrap();

        assert!(store.authenticate(&[1; 32], now).await.unwrap().is_none());
        assert!(store.authenticate(&[2; 32], now).await.unwrap().is_some());
        assert!(store.authenticate(&[3; 32], now).await.unwrap().is_some());
    }
}
