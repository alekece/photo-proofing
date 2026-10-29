use pp_core::auth::{AccountId, Identity, WorkspaceId};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

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

    pub async fn setup_available(&self) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM accounts)")
            .fetch_one(&self.pool)
            .await
    }

    /// A conflict rolls back the newly inserted workspace as well as the account.
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

    pub async fn authenticate(
        &self,
        token_hash: &[u8; 32],
        now: OffsetDateTime,
    ) -> Result<Option<Identity>, sqlx::Error> {
        let row: Option<IdentityRow> = sqlx::query_as("SELECT a.id AS account_id,a.workspace_id FROM sessions s JOIN accounts a ON a.id=s.account_id WHERE s.token_hash=$1 AND s.expires_at>$2")
            .bind(token_hash.as_slice()).bind(now).fetch_optional(&self.pool).await?;

        Ok(row.map(Into::into))
    }

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

    pub async fn revoke(&self, token_hash: &[u8; 32]) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
            .bind(token_hash.as_slice())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn cleanup_expired(
        &self,
        now: OffsetDateTime,
        limit: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE token_hash IN (SELECT token_hash FROM sessions WHERE expires_at<=$1 ORDER BY expires_at LIMIT $2)").bind(now).bind(limit).execute(&self.pool).await?;

        Ok(())
    }
}
