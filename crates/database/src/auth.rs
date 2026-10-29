//! Persistence for the single account and its revocable sessions.
//!
//! A session is a row keyed by the SHA-256 digest of the cookie token, with an absolute
//! `expires_at`. Revoking deletes the row, so copied tokens stop working immediately.
//! Expiry is sliding: renewal pushes `expires_at` forward, with no maximum age.

use pp_core::auth::{AccountId, Identity, WorkspaceId};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

/// Data access for the account and its sessions; see the module docs for the session model.
#[derive(Clone)]
pub struct AuthStore {
    pool: PgPool,
}

#[derive(FromRow)]
pub struct Account {
    pub id: Uuid,
    pub password_hash: String,
}

#[derive(FromRow)]
struct IdentityRow {
    account_id: Uuid,
    workspace_id: Uuid,
}

impl From<IdentityRow> for Identity {
    fn from(row: IdentityRow) -> Self {
        Self {
            account_id: AccountId(row.account_id),
            workspace_id: WorkspaceId(row.workspace_id),
        }
    }
}

impl AuthStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// True while no account exists, i.e. while the login page should offer setup.
    pub async fn setup_available(&self) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM accounts)")
            .fetch_one(&self.pool)
            .await
    }

    /// Creates the workspace, account and first session in one transaction. Returns `None`
    /// when an account already exists; the `singleton` unique column makes concurrent
    /// setups safe, and a conflict rolls back the newly inserted workspace as well.
    pub async fn create_account(
        &self,
        email: &str,
        password_hash: &str,
        token_hash: &[u8; 32],
        expires_at: OffsetDateTime,
    ) -> Result<Option<Identity>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let workspace_id = Uuid::new_v4();
        let account_id = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(&mut *tx)
            .await?;

        let inserted = sqlx::query("INSERT INTO accounts(id,workspace_id,email,password_hash) VALUES ($1,$2,$3,$4) ON CONFLICT DO NOTHING")
            .bind(account_id).bind(workspace_id).bind(email).bind(password_hash).execute(&mut *tx).await?.rows_affected();

        if inserted == 0 {
            tx.rollback().await?;

            return Ok(None);
        }

        sqlx::query("INSERT INTO sessions(token_hash,account_id,expires_at) VALUES ($1,$2,$3)")
            .bind(token_hash.as_slice())
            .bind(account_id)
            .bind(expires_at)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(Some(Identity {
            account_id: AccountId(account_id),
            workspace_id: WorkspaceId(workspace_id),
        }))
    }

    pub async fn account_by_email(&self, email: &str) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as("SELECT id,password_hash FROM accounts WHERE email=$1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
    }

    /// Inserts a session, first deleting `replaced_hash` (the caller's previous session on
    /// the same account) so repeated logins from one browser do not accumulate rows.
    /// Other devices' sessions are untouched.
    pub async fn create_session(
        &self,
        account_id: Uuid,
        token_hash: &[u8; 32],
        expires_at: OffsetDateTime,
        replaced_hash: Option<&[u8; 32]>,
    ) -> Result<(), sqlx::Error> {
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
            .bind(expires_at)
            .execute(&mut *tx)
            .await?;

        tx.commit().await
    }

    /// Read-only: identifies the session owner if it has not expired. Never extends expiry.
    pub async fn authenticate(
        &self,
        token_hash: &[u8; 32],
        now: OffsetDateTime,
    ) -> Result<Option<Identity>, sqlx::Error> {
        let row: Option<IdentityRow> = sqlx::query_as("SELECT a.id AS account_id,a.workspace_id FROM sessions s JOIN accounts a ON a.id=s.account_id WHERE s.token_hash=$1 AND s.expires_at>$2")
            .bind(token_hash.as_slice()).bind(now).fetch_optional(&self.pool).await?;

        Ok(row.map(Into::into))
    }

    /// Extends a live session and returns its owner, or `None` if it is expired or revoked.
    /// `GREATEST` means a late or out-of-order renewal can never shorten the expiry.
    pub async fn renew(
        &self,
        token_hash: &[u8; 32],
        now: OffsetDateTime,
        expires_at: OffsetDateTime,
    ) -> Result<Option<Identity>, sqlx::Error> {
        // UPDATE, never UPSERT: a revoked session must not be recreated by renewal.
        let row: Option<IdentityRow> = sqlx::query_as("UPDATE sessions s SET expires_at=GREATEST(s.expires_at,$3) FROM accounts a WHERE s.account_id=a.id AND s.token_hash=$1 AND s.expires_at>$2 RETURNING a.id AS account_id,a.workspace_id")
            .bind(token_hash.as_slice()).bind(now).bind(expires_at).fetch_optional(&self.pool).await?;

        Ok(row.map(Into::into))
    }

    /// Deletes the session; copies of its cookie stop authenticating at once. Idempotent.
    pub async fn revoke(&self, token_hash: &[u8; 32]) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(token_hash.as_slice())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    /// Housekeeping only: expired sessions already fail `authenticate`, so deleting at most
    /// `limit` per call just bounds table growth without long locks.
    pub async fn cleanup_expired(
        &self,
        now: OffsetDateTime,
        limit: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE expires_at<=$1 ORDER BY expires_at LIMIT $2)").bind(now).bind(limit).execute(&self.pool).await?;

        Ok(())
    }
}
