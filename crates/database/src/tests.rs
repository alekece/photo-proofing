//! Test-only setup and inspection for states the application never creates or reads through
//! its repositories. Compiled for this crate's tests and, through the `tests` feature, for the
//! tests of crates that enable it as a dev-dependency.

use std::time::Duration;

use pp_core::notifications::PhotoId;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use time::OffsetDateTime;
use tokio::time::sleep;

use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::GalleryId,
    jobs::{JobId, JobState},
    photo::PhotoStatus,
};

/// How often [`TestRepository::wait_for_a_lock_waiter`] polls.
const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// A job as stored, for tests that check what was enqueued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredJob {
    pub state: JobState,
    /// The tagged payload, `{"kind": …, "payload": …}`.
    pub payload: Value,
}

/// Holds an exclusive lock on the sessions table until dropped, so session lookups block.
pub struct SessionsLock(#[allow(dead_code)] Transaction<'static, Postgres>);

pub struct TestRepository<'db> {
    pool: &'db PgPool,
}

impl<'db> Repository<'db> for TestRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): ()) -> Self {
        Self { pool }
    }
}

impl TestRepository<'_> {
    /// Returns once some session of this database waits on a lock, to order concurrent steps.
    pub async fn wait_for_a_lock_waiter(&self) -> Result<(), DatabaseError> {
        loop {
            let lock_waiters: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_stat_activity
                 WHERE datname = current_database() AND wait_event_type = 'Lock'",
            )
            .fetch_one(self.pool)
            .await?;

            if lock_waiters > 0 {
                return Ok(());
            }

            sleep(LOCK_POLL_INTERVAL).await;
        }
    }

    /// Whether the migrations created the job table yet.
    pub async fn is_migrated(&self) -> Result<bool, DatabaseError> {
        Ok(sqlx::query_scalar("SELECT to_regclass('jobs') IS NOT NULL")
            .fetch_one(self.pool)
            .await?)
    }

    /// A workspace with no account, for cross-workspace and fixture tests.
    pub async fn create_workspace(&self) -> Result<WorkspaceId, DatabaseError> {
        Ok(
            sqlx::query_scalar(
                "INSERT INTO workspaces(id) VALUES (gen_random_uuid()) RETURNING id",
            )
            .fetch_one(self.pool)
            .await?,
        )
    }

    /// Deletes a gallery and, by cascade, its photos. Their jobs remain: they name the photo only
    /// in their payload.
    pub async fn delete_gallery(&self, id: GalleryId) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM galleries WHERE id = $1")
            .bind(id)
            .execute(self.pool)
            .await?;

        Ok(())
    }

    /// Sets a photo's status regardless of its current one, including moving it backwards.
    pub async fn force_photo_status(
        &self,
        id: PhotoId,
        status: &PhotoStatus,
    ) -> Result<(), DatabaseError> {
        let (status, failure_reason) = match status {
            PhotoStatus::Waiting => ("waiting", None),
            PhotoStatus::Processing => ("processing", None),
            PhotoStatus::Ready => ("ready", None),
            PhotoStatus::Failed(reason) => ("failed", Some(reason.as_str())),
        };

        sqlx::query(
            "UPDATE photos SET status = $2::photo_status, failure_reason = $3 WHERE id = $1",
        )
        .bind(id)
        .bind(status)
        .bind(failure_reason)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    pub async fn set_photo_created_at(
        &self,
        id: PhotoId,
        created_at: OffsetDateTime,
    ) -> Result<(), DatabaseError> {
        sqlx::query("UPDATE photos SET created_at = $2 WHERE id = $1")
            .bind(id)
            .bind(created_at)
            .execute(self.pool)
            .await?;

        Ok(())
    }

    pub async fn count_photos(&self) -> Result<i64, DatabaseError> {
        Ok(sqlx::query_scalar("SELECT count(*) FROM photos")
            .fetch_one(self.pool)
            .await?)
    }

    /// Counts the jobs in `state`, or all jobs.
    pub async fn count_jobs(&self, state: Option<JobState>) -> Result<i64, DatabaseError> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM jobs WHERE $1::job_state IS NULL OR state = $1",
        )
        .bind(state)
        .fetch_one(self.pool)
        .await?)
    }

    /// The jobs whose payload names `photo_id`.
    pub async fn photo_jobs(&self, photo_id: PhotoId) -> Result<Vec<StoredJob>, DatabaseError> {
        let rows: Vec<(JobState, Value)> = sqlx::query_as(
            "SELECT state, payload FROM jobs WHERE payload->'payload'->>'photo_id' = $1",
        )
        .bind(photo_id.to_string())
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(state, payload)| StoredJob { state, payload })
            .collect())
    }

    /// The jobs whose payload has the given `kind` tag, oldest first.
    pub async fn jobs_of_kind(&self, kind: &str) -> Result<Vec<StoredJob>, DatabaseError> {
        let rows: Vec<(JobState, Value)> = sqlx::query_as(
            "SELECT state, payload FROM jobs WHERE payload->>'kind' = $1 ORDER BY created_at, id",
        )
        .bind(kind)
        .fetch_all(self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(state, payload)| StoredJob { state, payload })
            .collect())
    }

    /// Queues a job with an arbitrary, possibly malformed, payload.
    pub async fn insert_raw_job(&self, payload: Value) -> Result<JobId, DatabaseError> {
        Ok(sqlx::query_scalar(
            "INSERT INTO jobs(id, payload, max_attempts) VALUES (gen_random_uuid(), $1, 5)
             RETURNING id",
        )
        .bind(payload)
        .fetch_one(self.pool)
        .await?)
    }

    pub async fn job_attempts(&self, id: JobId) -> Result<i32, DatabaseError> {
        Ok(
            sqlx::query_scalar("SELECT attempts FROM jobs WHERE id = $1")
                .bind(id)
                .fetch_one(self.pool)
                .await?,
        )
    }

    /// The expiry of the only session.
    pub async fn session_expiry(&self) -> Result<OffsetDateTime, DatabaseError> {
        Ok(sqlx::query_scalar("SELECT expires_at FROM sessions")
            .fetch_one(self.pool)
            .await?)
    }

    pub async fn expire_sessions(&self) -> Result<(), DatabaseError> {
        sqlx::query("UPDATE sessions SET expires_at = now() - interval '1 second'")
            .execute(self.pool)
            .await?;

        Ok(())
    }

    pub async fn lock_sessions(&self) -> Result<SessionsLock, DatabaseError> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query("LOCK TABLE sessions IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *transaction)
            .await?;

        Ok(SessionsLock(transaction))
    }

    /// Makes every later job insert fail, as a database fault would.
    pub async fn reject_job_inserts(&self) -> Result<(), DatabaseError> {
        sqlx::query(
            "CREATE FUNCTION reject_job_insert() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RAISE EXCEPTION 'injected test failure'; END; $$",
        )
        .execute(self.pool)
        .await?;
        sqlx::query(
            "CREATE TRIGGER reject_job_insert BEFORE INSERT ON jobs
             FOR EACH ROW EXECUTE FUNCTION reject_job_insert()",
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Drops the connection while committing any photo insert, so the caller cannot know
    /// whether the commit happened.
    pub async fn interrupt_photo_commits(&self) -> Result<(), DatabaseError> {
        sqlx::query(
            "CREATE FUNCTION interrupt_photo_commit() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN PERFORM pg_terminate_backend(pg_backend_pid()); RETURN NEW; END; $$",
        )
        .execute(self.pool)
        .await?;
        sqlx::query(
            "CREATE CONSTRAINT TRIGGER interrupt_photo_commit AFTER INSERT ON photos
             DEFERRABLE INITIALLY DEFERRED
             FOR EACH ROW EXECUTE FUNCTION interrupt_photo_commit()",
        )
        .execute(self.pool)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod connection {
    use std::error::Error as _;

    use crate::{Database, error::DatabaseError};

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
