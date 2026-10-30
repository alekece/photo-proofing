//! Persistence for the single account and its revocable sessions.
//!
//! A session is a row keyed by the SHA-256 digest of the cookie token, with an absolute
//! `expires_at`. Revoking deletes the row, so copied tokens stop working immediately.
//! Expiry is sliding: renewal pushes `expires_at` forward, with no maximum age.

use pp_core::{
    auth::{AccountId, Identity, WorkspaceId},
    email::Email,
};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::Repository;

/// Data access for the account and its sessions; see the module docs for the session model.
#[derive(Clone, Copy)]
pub struct AuthRepository<'db> {
    pool: &'db PgPool,
}

#[derive(FromRow)]
pub struct Account {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub password_hash: String,
}

impl Account {
    pub fn identity(&self) -> Identity {
        Identity {
            account_id: AccountId(self.id),
            workspace_id: WorkspaceId(self.workspace_id),
        }
    }
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

impl<'db> Repository<'db> for AuthRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl AuthRepository<'_> {
    /// True while no account exists, i.e. while the login page should offer setup.
    pub async fn setup_available(&self) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM accounts)")
            .fetch_one(self.pool)
            .await
    }

    /// Creates the workspace, account and first session in one transaction. Returns `None`
    /// when an account already exists; the `singleton` unique column makes concurrent
    /// setups safe, and a conflict rolls back the newly inserted workspace as well.
    pub async fn create_account(
        &self,
        email: &Email,
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
            .bind(account_id).bind(workspace_id).bind(email.as_str()).bind(password_hash).execute(&mut *tx).await?.rows_affected();

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

    pub async fn account_by_email(&self, email: &Email) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as("SELECT id,workspace_id,password_hash FROM accounts WHERE email=$1")
            .bind(email.as_str())
            .fetch_optional(self.pool)
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
            .bind(token_hash.as_slice()).bind(now).fetch_optional(self.pool).await?;

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
            .bind(token_hash.as_slice()).bind(now).bind(expires_at).fetch_optional(self.pool).await?;

        Ok(row.map(Into::into))
    }

    /// Deletes the session; copies of its cookie stop authenticating at once. Idempotent.
    pub async fn revoke(&self, token_hash: &[u8; 32]) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(token_hash.as_slice())
            .execute(self.pool)
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
        sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE expires_at<=$1 ORDER BY expires_at LIMIT $2)").bind(now).bind(limit).execute(self.pool).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use time::Duration;

    use super::*;
    use crate::Database;

    fn email(value: &str) -> Email {
        value.parse().unwrap()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn setup_session_and_revocation_are_persistent(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<AuthRepository>();
        let now = OffsetDateTime::now_utc();

        assert!(store.setup_available().await.unwrap());

        let identity = store
            .create_account(
                &email("person@example.test"),
                "hash",
                &[1; 32],
                now + Duration::days(30),
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
                    now + Duration::days(30)
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
                .authenticate(&[1; 32], now + Duration::days(30))
                .await
                .unwrap()
                .is_none()
        );

        store
            .renew(&[1; 32], now, now + Duration::days(40))
            .await
            .unwrap()
            .unwrap();

        assert!(
            store
                .authenticate(&[1; 32], now + Duration::days(31))
                .await
                .unwrap()
                .is_some()
        );

        store.revoke(&[1; 32]).await.unwrap();

        assert!(
            store
                .renew(&[1; 32], now, now + Duration::days(50))
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
        let expiry = OffsetDateTime::now_utc() + Duration::days(30);
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
            let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
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
        let now = OffsetDateTime::now_utc();

        store
            .create_account(
                &email("a@example.test"),
                "hash",
                &[1; 32],
                now + Duration::days(1),
            )
            .await
            .unwrap();

        let (renew, revoke) = tokio::join!(
            store.renew(&[1; 32], now, now + Duration::days(2)),
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
        let now = OffsetDateTime::now_utc();
        let id = store
            .create_account(
                &email("a@example.test"),
                "hash",
                &[1; 32],
                now + Duration::days(1),
            )
            .await
            .unwrap()
            .unwrap();

        store
            .create_session(id.account_id.0, &[2; 32], now + Duration::days(1), None)
            .await
            .unwrap();
        store
            .create_session(
                id.account_id.0,
                &[3; 32],
                now + Duration::days(1),
                Some(&[1; 32]),
            )
            .await
            .unwrap();

        assert!(store.authenticate(&[1; 32], now).await.unwrap().is_none());
        assert!(store.authenticate(&[2; 32], now).await.unwrap().is_some());
        assert!(store.authenticate(&[3; 32], now).await.unwrap().is_some());
    }
}
