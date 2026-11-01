//! The durable job queue.
//!
//! A job is a row in `jobs`; the queue is PostgreSQL only (TC-04). The life of a job:
//!
//! - [`enqueue`](JobRepository::enqueue) stores it `queued`.
//! - [`claim`](JobRepository::claim) atomically moves one due job to `running`, bumps
//!   `attempts` and sets a lease (`locked_until`). Concurrent claimers skip each other's rows,
//!   so a job is never handed out twice at once. A `running` job whose lease has passed is
//!   claimable again: that is how work interrupted by a crash is recovered.
//! - The worker extends the lease with [`renew`](JobRepository::renew) while it works, then
//!   calls [`complete`](JobRepository::complete) (the row is deleted; the domain row carries the
//!   user-visible status) or [`fail`](JobRepository::fail) (requeued with backoff, or `failed`
//!   once `max_attempts` is reached). A `failed` job stays until
//!   [`retry`](JobRepository::retry) requeues it.
//!
//! `attempts` is incremented at claim, not at failure, so a job that kills its worker without
//! reporting still reaches the cap: an expired lease on the final attempt marks the job
//! `failed` instead of running it again.
//!
//! `renew`, `complete`, `fail` and [`abandon`](JobRepository::abandon) take the [`Job`] returned
//! by `claim` and only act while that claim is still current (the job is `running` with the same
//! `attempts`). A worker that lost its lease to another one therefore cannot overwrite the new
//! run's outcome; they report `false`/`None` so the caller can stop.
//!
//! Handlers must be idempotent: a job may run again after a crash or a lost lease, and nothing
//! here ties a job to its subject, so deletion safety is the handler's job.

use std::{num::NonZeroU16, time::Duration};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Error as JsonError, Value};
use sqlx::{FromRow, PgPool, Row, Type, postgres::PgRow};
use uuid::Uuid;

use crate::{Repository, error::DatabaseError};

/// Longest `last_error` kept, in characters; the full error belongs in the logs.
const MAX_ERROR_CHARS: usize = 2000;

/// Attempts a job gets when the caller has no reason to choose another number.
pub const DEFAULT_MAX_ATTEMPTS: NonZeroU16 = NonZeroU16::new(5).unwrap();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct JobId(pub Uuid);

/// What a job does, which selects its handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    ProcessPhoto,
}

impl JobKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ProcessPhoto => "process_photo",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "process_photo" => Some(Self::ProcessPhoto),
            _ => None,
        }
    }
}

/// A durable job's tag and body, stored together in the `payload` JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum JobPayload {
    /// The body remains opaque until the photo-processing contract is defined.
    ProcessPhoto(Value),
}

impl JobPayload {
    /// Returns the discriminator encoded with this payload.
    pub const fn kind(&self) -> JobKind {
        match self {
            Self::ProcessPhoto(_) => JobKind::ProcessPhoto,
        }
    }

    fn body(&self) -> &Value {
        match self {
            Self::ProcessPhoto(body) => body,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "job_state", rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    /// Out of attempts; kept until a manual [`retry`](JobRepository::retry).
    Failed,
}

/// A claimed job. Hand it back to the repository to renew, complete or fail the claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: JobId,
    pub payload: JobPayload,
    /// Including the current run, so the first run sees `1`.
    pub attempts: u32,
    pub max_attempts: u32,
}

impl Job {
    /// Returns the discriminator encoded with this job's payload.
    pub const fn kind(&self) -> JobKind {
        self.payload.kind()
    }

    /// Reads the payload as the type the handler for this kind expects.
    pub fn payload<T: DeserializeOwned>(&self) -> Result<T, JsonError> {
        serde_json::from_value(self.payload.body().clone())
    }
}

impl FromRow<'_, PgRow> for Job {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let count = |column: &str| {
            u32::try_from(row.try_get::<i32, _>(column)?)
                .map_err(|_| sqlx::Error::Decode(format!("{column} is negative").into()))
        };

        Ok(Self {
            id: row.try_get("id")?,
            payload: serde_json::from_value(row.try_get("payload")?)
                .map_err(|error| sqlx::Error::Decode(error.into()))?,
            attempts: count("attempts")?,
            max_attempts: count("max_attempts")?,
        })
    }
}

/// A job as seen by an operator, without the claim bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobStatus {
    pub id: JobId,
    pub kind: JobKind,
    pub state: JobState,
    pub last_error: Option<String>,
}

impl FromRow<'_, PgRow> for JobStatus {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let tag = row.try_get::<String, _>("kind")?;
        let kind = JobKind::from_tag(&tag)
            .ok_or_else(|| sqlx::Error::Decode(format!("unknown job kind: {tag}").into()))?;

        Ok(Self {
            id: row.try_get("id")?,
            kind,
            state: row.try_get("state")?,
            last_error: row.try_get("last_error")?,
        })
    }
}

/// Exponential retry delay: `base`, `2 * base`, `4 * base`, ... never above `cap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    base: Duration,
    cap: Duration,
}

impl Backoff {
    pub fn new(base: Duration, cap: Duration) -> Self {
        Self {
            base,
            cap: cap.max(base),
        }
    }

    /// The wait before retrying after the failure of attempt number `attempt` (1-based).
    pub fn delay(&self, attempt: u32) -> Duration {
        let factor = 1u32
            .checked_shl(attempt.saturating_sub(1))
            .unwrap_or(u32::MAX);

        self.base.saturating_mul(factor).min(self.cap)
    }
}

pub struct JobRepository<'db> {
    pool: &'db PgPool,
}

impl<'db> Repository<'db> for JobRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl JobRepository<'_> {
    const COLUMNS: &'static str = "id, payload, attempts, max_attempts";

    /// Stores a job, due immediately.
    pub async fn enqueue(
        &self,
        payload: JobPayload,
        max_attempts: NonZeroU16,
    ) -> Result<JobId, DatabaseError> {
        let id = JobId(Uuid::new_v4());
        let payload = serde_json::to_value(payload).map_err(|e| sqlx::Error::Encode(e.into()))?;

        sqlx::query("INSERT INTO jobs(id, payload, max_attempts) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(payload)
            .bind(i32::from(max_attempts.get()))
            .execute(self.pool)
            .await?;

        Ok(id)
    }

    /// Claims the oldest due job among `kinds`, leasing it for `lease`.
    ///
    /// Jobs of other kinds stay queued, so a worker without a handler for a kind never touches
    /// it. Jobs whose lease expired on their final attempt are marked failed on the way.
    pub async fn claim(
        &self,
        kinds: &[JobKind],
        lease: Duration,
    ) -> Result<Option<Job>, DatabaseError> {
        let kinds = kinds
            .iter()
            .map(|kind| kind.as_str().to_owned())
            .collect::<Vec<_>>();

        sqlx::query(
            "UPDATE jobs SET state = 'failed', locked_until = NULL,
                 last_error = 'the worker stopped answering on the final attempt'
             WHERE id IN (
                 SELECT id FROM jobs
                 WHERE state = 'running' AND locked_until < now() AND attempts >= max_attempts
                 FOR UPDATE SKIP LOCKED)",
        )
        .execute(self.pool)
        .await?;

        Ok(sqlx::query_as(&format!(
            "UPDATE jobs SET state = 'running', attempts = attempts + 1,
                 locked_until = now() + $2 * interval '1 second'
             WHERE id = (
                 SELECT id FROM jobs
                 WHERE payload->>'kind' = ANY($1) AND attempts < max_attempts
                   AND ((state = 'queued' AND run_at <= now())
                     OR (state = 'running' AND locked_until < now()))
                 ORDER BY run_at
                 LIMIT 1
                 FOR UPDATE SKIP LOCKED)
             RETURNING {}",
            Self::COLUMNS
        ))
        .bind(kinds)
        .bind(lease.as_secs_f64())
        .fetch_optional(self.pool)
        .await?)
    }

    /// Extends the lease to `lease` from now. `false` means the claim is no longer current.
    pub async fn renew(&self, job: &Job, lease: Duration) -> Result<bool, DatabaseError> {
        let done = sqlx::query(
            "UPDATE jobs SET locked_until = now() + $3 * interval '1 second'
             WHERE id = $1 AND state = 'running' AND attempts = $2",
        )
        .bind(job.id)
        .bind(attempts_param(job))
        .bind(lease.as_secs_f64())
        .execute(self.pool)
        .await?;

        Ok(done.rows_affected() == 1)
    }

    /// Deletes a finished job. `false` means the claim was no longer current.
    pub async fn complete(&self, job: &Job) -> Result<bool, DatabaseError> {
        let done =
            sqlx::query("DELETE FROM jobs WHERE id = $1 AND state = 'running' AND attempts = $2")
                .bind(job.id)
                .bind(attempts_param(job))
                .execute(self.pool)
                .await?;

        Ok(done.rows_affected() == 1)
    }

    /// Records a recoverable failure: the job is requeued after `backoff`, or becomes
    /// [`Failed`](JobState::Failed) when this was its last attempt.
    ///
    /// Returns the new state, or `None` if the claim was no longer current.
    pub async fn fail(
        &self,
        job: &Job,
        error: &str,
        backoff: Backoff,
    ) -> Result<Option<JobState>, DatabaseError> {
        Ok(sqlx::query_scalar(
            "UPDATE jobs SET
                 state = CASE WHEN attempts >= max_attempts
                     THEN 'failed'::job_state ELSE 'queued'::job_state END,
                 run_at = CASE WHEN attempts >= max_attempts
                     THEN run_at ELSE now() + $4 * interval '1 second' END,
                 locked_until = NULL,
                 last_error = $3
             WHERE id = $1 AND state = 'running' AND attempts = $2
             RETURNING state",
        )
        .bind(job.id)
        .bind(attempts_param(job))
        .bind(clean_error(error))
        .bind(backoff.delay(job.attempts).as_secs_f64())
        .fetch_optional(self.pool)
        .await?)
    }

    /// Fails the job for good, regardless of remaining attempts, for errors a retry cannot fix.
    /// `false` means the claim was no longer current.
    pub async fn abandon(&self, job: &Job, error: &str) -> Result<bool, DatabaseError> {
        let done = sqlx::query(
            "UPDATE jobs SET state = 'failed', locked_until = NULL, last_error = $3
             WHERE id = $1 AND state = 'running' AND attempts = $2",
        )
        .bind(job.id)
        .bind(attempts_param(job))
        .bind(clean_error(error))
        .execute(self.pool)
        .await?;

        Ok(done.rows_affected() == 1)
    }

    /// Manually requeues a failed job with a fresh set of attempts. `false` means there is no
    /// failed job with this id.
    pub async fn retry(&self, id: JobId) -> Result<bool, DatabaseError> {
        let done = sqlx::query(
            "UPDATE jobs SET state = 'queued', attempts = 0, run_at = now(), last_error = NULL
             WHERE id = $1 AND state = 'failed'",
        )
        .bind(id)
        .execute(self.pool)
        .await?;

        Ok(done.rows_affected() == 1)
    }

    /// Jobs that ran out of attempts, newest first, for operators and later UI.
    pub async fn list_failed(&self) -> Result<Vec<JobStatus>, DatabaseError> {
        Ok(sqlx::query_as(
            "SELECT id, payload->>'kind' AS kind, state, last_error FROM jobs
             WHERE state = 'failed' ORDER BY created_at DESC",
        )
        .fetch_all(self.pool)
        .await?)
    }
}

/// `attempts` fits `i32` because it was read from an `integer` column.
fn attempts_param(job: &Job) -> i32 {
    i32::try_from(job.attempts).unwrap_or(i32::MAX)
}

/// PostgreSQL text cannot hold NUL, and an unbounded error would bloat the row.
fn clean_error(error: &str) -> String {
    error
        .chars()
        .filter(|c| *c != '\0')
        .take(MAX_ERROR_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, time::Duration};

    use serde_json::json;
    use tokio::{task::JoinSet, time};

    use super::*;
    use crate::Database;

    const LEASE: Duration = Duration::from_millis(300);
    const BACKOFF: Backoff = Backoff {
        base: Duration::from_millis(100),
        cap: Duration::from_millis(400),
    };
    const KINDS: &[JobKind] = &[JobKind::ProcessPhoto];

    async fn enqueue(database: &Database, max_attempts: u16) -> JobId {
        database
            .repository::<JobRepository>()
            .enqueue(
                JobPayload::ProcessPhoto(json!({"photo": 1})),
                NonZeroU16::new(max_attempts).unwrap(),
            )
            .await
            .unwrap()
    }

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let backoff = Backoff::new(Duration::from_secs(10), Duration::from_secs(60));
        let delays: Vec<_> = (1..=5).map(|n| backoff.delay(n).as_secs()).collect();

        assert_eq!(delays, [10, 20, 40, 60, 60]);
        assert_eq!(backoff.delay(u32::MAX), Duration::from_secs(60));
    }

    #[test]
    fn job_payload_serializes_its_tag_and_body_together() {
        let payload = JobPayload::ProcessPhoto(json!({"photo": 1}));

        assert_eq!(
            serde_json::to_value(payload).unwrap(),
            json!({"kind": "process_photo", "payload": {"photo": 1}})
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn concurrent_claimers_never_share_a_job(pool: PgPool) {
        let database = Database::new(pool);
        for _ in 0..30 {
            enqueue(&database, 3).await;
        }

        let mut claimers = JoinSet::new();
        for _ in 0..8 {
            let database = database.clone();
            claimers.spawn(async move {
                let jobs = database.repository::<JobRepository>();
                let mut mine = Vec::new();
                while let Some(job) = jobs.claim(KINDS, Duration::from_secs(60)).await.unwrap() {
                    mine.push(job.id);
                }
                mine
            });
        }
        let mut all = Vec::new();
        while let Some(mine) = claimers.join_next().await {
            all.extend(mine.unwrap());
        }

        assert_eq!(all.len(), 30);
        assert_eq!(all.iter().collect::<HashSet<_>>().len(), 30);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn live_lease_is_respected_and_expired_lease_is_reclaimed(pool: PgPool) {
        let database = Database::new(pool);
        let jobs = database.repository::<JobRepository>();
        let id = enqueue(&database, 3).await;

        let first = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        assert_eq!((first.id, first.attempts), (id, 1));
        assert!(jobs.claim(KINDS, LEASE).await.unwrap().is_none());

        time::sleep(LEASE + Duration::from_millis(100)).await;
        let second = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        assert_eq!((second.id, second.attempts), (id, 2));
        // The first claimer lost the job and can neither renew nor finish it.
        assert!(!jobs.renew(&first, LEASE).await.unwrap());
        assert!(!jobs.complete(&first).await.unwrap());
        assert!(jobs.complete(&second).await.unwrap());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn renewing_keeps_the_lease_alive(pool: PgPool) {
        let database = Database::new(pool);
        let jobs = database.repository::<JobRepository>();
        enqueue(&database, 3).await;

        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        for _ in 0..3 {
            time::sleep(LEASE / 2).await;
            assert!(jobs.renew(&job, LEASE).await.unwrap());
        }

        assert!(jobs.claim(KINDS, LEASE).await.unwrap().is_none());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn failures_back_off_then_stop_visibly_at_the_cap(pool: PgPool) {
        let database = Database::new(pool);
        let jobs = database.repository::<JobRepository>();
        let id = enqueue(&database, 2).await;

        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        assert_eq!(
            jobs.fail(&job, "boom", BACKOFF).await.unwrap(),
            Some(JobState::Queued)
        );
        assert!(jobs.claim(KINDS, LEASE).await.unwrap().is_none());

        time::sleep(BACKOFF.delay(1) + Duration::from_millis(50)).await;
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        assert_eq!(
            jobs.fail(&job, "boom again", BACKOFF).await.unwrap(),
            Some(JobState::Failed)
        );

        let failed = jobs.list_failed().await.unwrap();
        assert_eq!(failed.len(), 1);
        assert_eq!(failed[0].id, id);
        assert_eq!(failed[0].kind, JobKind::ProcessPhoto);
        assert_eq!(failed[0].last_error.as_deref(), Some("boom again"));
        assert!(jobs.claim(KINDS, LEASE).await.unwrap().is_none());

        assert!(jobs.retry(id).await.unwrap());
        assert!(!jobs.retry(id).await.unwrap());
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        assert_eq!((job.id, job.attempts), (id, 1));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn crash_looping_job_stops_at_the_cap(pool: PgPool) {
        let database = Database::new(pool);
        let jobs = database.repository::<JobRepository>();
        let id = enqueue(&database, 2).await;

        // Each claimer "crashes": it never completes, renews or fails the job.
        for attempt in 1..=2 {
            let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
            assert_eq!((job.id, job.attempts), (id, attempt));
            time::sleep(LEASE + Duration::from_millis(100)).await;
        }

        assert!(jobs.claim(KINDS, LEASE).await.unwrap().is_none());
        let failed = jobs.list_failed().await.unwrap();
        assert_eq!(failed.iter().map(|job| job.id).collect::<Vec<_>>(), [id]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn only_requested_kinds_are_claimed(pool: PgPool) {
        let database = Database::new(pool);
        let jobs = database.repository::<JobRepository>();
        enqueue(&database, 3).await;

        assert!(jobs.claim(&[], LEASE).await.unwrap().is_none());
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        assert_eq!(job.kind(), JobKind::ProcessPhoto);
        assert_eq!(job.payload::<Value>().unwrap(), json!({"photo": 1}));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn payload_migration_preserves_existing_job_data(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("job_payload_migration_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(&format!("SET LOCAL search_path TO {schema}"))
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0004_jobs.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();

        let id = Uuid::new_v4();
        let body = json!({"photo": 1});
        sqlx::query(
            "INSERT INTO jobs
                 (id, kind, payload, state, attempts, max_attempts, run_at,
                  locked_until, last_error, created_at)
             VALUES
                 ($1, 'process_photo', $2, 'running', 2, 5,
                  '2026-10-31 10:00:00+00', '2026-11-01 10:00:00+00',
                  'previous error', '2026-10-30 10:00:00+00')",
        )
        .bind(id)
        .bind(&body)
        .execute(&mut *transaction)
        .await
        .unwrap();

        let before: Value = sqlx::query_scalar("SELECT to_jsonb(j) FROM jobs AS j")
            .fetch_one(&mut *transaction)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0005_job_payload_enum.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();
        let after: Value = sqlx::query_scalar("SELECT to_jsonb(j) FROM jobs AS j")
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        assert_eq!(
            after["payload"],
            json!({"kind": "process_photo", "payload": body})
        );
        let mut before_fields = before.as_object().unwrap().clone();
        before_fields.remove("kind");
        before_fields.remove("payload");
        let mut after_fields = after.as_object().unwrap().clone();
        after_fields.remove("payload");
        assert_eq!(after_fields, before_fields);

        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn abandon_fails_regardless_of_remaining_attempts(pool: PgPool) {
        let database = Database::new(pool);
        let jobs = database.repository::<JobRepository>();
        enqueue(&database, 5).await;

        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        assert!(jobs.abandon(&job, "bad payload\0").await.unwrap());
        assert_eq!(
            jobs.list_failed().await.unwrap()[0].last_error.as_deref(),
            Some("bad payload")
        );
    }
}
