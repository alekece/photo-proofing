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

use std::{
    num::{NonZeroU16, NonZeroUsize},
    time::Duration,
};

use pp_core::{notifications::PhotoId, types::generate::Generate};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Error as JsonError, Value};
use sqlx::{
    AssertSqlSafe, FromRow, PgExecutor, PgPool, Postgres, Row, Transaction, Type, postgres::PgRow,
};
use strum::{EnumString, IntoStaticStr};
use uuid::Uuid;

use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::GalleryId,
    photo::{PROCESSING_FAILED_REASON, PhotoRepository},
};

/// Longest `last_error` kept, in characters; the full error belongs in the logs.
const MAX_ERROR_CHARS: usize = 2000;

/// Attempts a job gets when the caller has no reason to choose another number.
pub const DEFAULT_MAX_ATTEMPTS: NonZeroU16 = NonZeroU16::new(5).unwrap();

/// Photos one cleanup job names unless configured otherwise.
pub const DEFAULT_CLEANUP_BATCH: NonZeroUsize = NonZeroUsize::new(500).unwrap();

/// How the jobs a repository enqueues are configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobOptions {
    /// Attempts each job gets before it fails for good.
    pub max_attempts: NonZeroU16,
    /// Most photos one cleanup job names, so a bulk delete never produces an oversized job.
    pub cleanup_batch: NonZeroUsize,
}

impl Default for JobOptions {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            cleanup_batch: DEFAULT_CLEANUP_BATCH,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, serde::Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct JobId(pub Uuid);

impl Generate for JobId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

/// What a job does, which selects its handler.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, EnumString, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum JobKind {
    ProcessPhoto,
    CleanupGallery,
    CleanupPhotos,
    SweepOrphans,
}

impl JobKind {
    /// The tag this kind has in a stored payload.
    pub fn as_str(self) -> &'static str {
        self.into()
    }
}

/// Turns one uploaded original into its derivatives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessPhoto {
    pub photo_id: PhotoId,
}

/// Removes every file of a deleted gallery. The ids alone locate them, so the job outlives the
/// rows it was created for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupGallery {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
}

/// Removes every file of photos deleted from one gallery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CleanupPhotos {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
    pub photo_ids: Vec<PhotoId>,
}

/// A durable job's tag and body, stored together in the `payload` JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum JobPayload {
    ProcessPhoto(ProcessPhoto),
    CleanupGallery(CleanupGallery),
    CleanupPhotos(CleanupPhotos),
    /// Removes photo files that no photo row owns.
    SweepOrphans,
}

impl From<ProcessPhoto> for JobPayload {
    fn from(payload: ProcessPhoto) -> Self {
        Self::ProcessPhoto(payload)
    }
}

impl From<CleanupGallery> for JobPayload {
    fn from(payload: CleanupGallery) -> Self {
        Self::CleanupGallery(payload)
    }
}

impl From<CleanupPhotos> for JobPayload {
    fn from(payload: CleanupPhotos) -> Self {
        Self::CleanupPhotos(payload)
    }
}

impl JobPayload {
    /// Returns the discriminator encoded with this payload.
    pub const fn kind(&self) -> JobKind {
        match self {
            Self::ProcessPhoto(_) => JobKind::ProcessPhoto,
            Self::CleanupGallery(_) => JobKind::CleanupGallery,
            Self::CleanupPhotos(_) => JobKind::CleanupPhotos,
            Self::SweepOrphans => JobKind::SweepOrphans,
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
///
/// The body stays raw JSON until a handler reads it, so a malformed body is still claimed and
/// abandoned by the worker instead of failing the claim itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: JobId,
    kind: JobKind,
    body: Value,
    /// Including the current run, so the first run sees `1`.
    pub attempts: u32,
    pub max_attempts: u32,
}

impl Job {
    /// Returns the discriminator encoded with this job's payload.
    pub const fn kind(&self) -> JobKind {
        self.kind
    }

    /// Reads the payload body as the type the handler for this kind expects.
    pub fn payload<T: DeserializeOwned>(&self) -> Result<T, JsonError> {
        T::deserialize(&self.body)
    }
}

impl FromRow<'_, PgRow> for Job {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let count = |column: &str| {
            u32::try_from(row.try_get::<i32, _>(column)?)
                .map_err(|_| sqlx::Error::Decode(format!("{column} is negative").into()))
        };
        let kind = row
            .try_get::<String, _>("kind")?
            .parse::<JobKind>()
            .map_err(|error| sqlx::Error::Decode(error.into()))?;

        Ok(Self {
            id: row.try_get("id")?,
            kind,
            body: row.try_get("body")?,
            attempts: count("attempts")?,
            max_attempts: count("max_attempts")?,
        })
    }
}

/// Proof that a job was finished; only [`JobRepository::complete`] and
/// [`JobRepository::complete_photo`] create one, so a handler cannot succeed without finishing
/// its job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub struct JobCompletion {
    current: bool,
}

impl JobCompletion {
    /// Whether the claim was still ours; `false` means another run owns the job now.
    pub const fn is_current(self) -> bool {
        self.current
    }
}

/// The final state a photo job gives its photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoResult<'a> {
    Ready,
    /// Failed for good, with a short reason safe to show the photographer.
    Failed(&'a str),
}

/// What [`JobRepository::complete_photo`] did to the photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhotoUpdate {
    /// The result was saved.
    Saved,
    /// The photo exists but was left as is: already finished, or owned by another run.
    Unchanged,
    /// The photo no longer exists.
    Missing,
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
        let kind = tag
            .parse::<JobKind>()
            .map_err(|error| sqlx::Error::Decode(error.into()))?;

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
    const COLUMNS: &'static str = "id, payload->>'kind' AS kind,
        COALESCE(payload->'payload', 'null'::jsonb) AS body, attempts, max_attempts";
    /// Photo targeted by a job, as text so a malformed payload cannot abort the statement.
    const PHOTO_ID: &'static str = "payload->'payload'->>'photo_id' AS photo_id";

    /// Stores a job, due immediately.
    pub async fn enqueue(
        &self,
        payload: impl Into<JobPayload>,
        max_attempts: NonZeroU16,
    ) -> Result<JobId, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let id = Self::enqueue_in(&mut transaction, payload, max_attempts).await?;

        transaction.commit().await?;

        Ok(id)
    }

    /// Stores a job in the caller's transaction, due immediately.
    pub async fn enqueue_in(
        transaction: &mut Transaction<'_, Postgres>,
        payload: impl Into<JobPayload>,
        max_attempts: NonZeroU16,
    ) -> Result<JobId, DatabaseError> {
        let id = JobId::generate();
        let payload =
            serde_json::to_value(payload.into()).map_err(|e| sqlx::Error::Encode(e.into()))?;

        sqlx::query("INSERT INTO jobs(id, payload, max_attempts) VALUES ($1, $2, $3)")
            .bind(id)
            .bind(payload)
            .bind(i32::from(max_attempts.get()))
            .execute(&mut **transaction)
            .await?;

        Ok(id)
    }

    /// Claims the oldest due job among `kinds`, leasing it for `lease`.
    ///
    /// Jobs of other kinds stay queued, so a worker without a handler for a kind never touches
    /// it. Jobs whose lease expired on their final attempt are marked failed on the way, together
    /// with their photo.
    pub async fn claim(
        &self,
        kinds: &[JobKind],
        lease: Duration,
    ) -> Result<Option<Job>, DatabaseError> {
        let kinds = kinds
            .iter()
            .map(|kind| kind.as_str().to_owned())
            .collect::<Vec<_>>();

        let mut transaction = self.pool.begin().await?;
        let expired_photos: Vec<Option<String>> = sqlx::query_scalar(AssertSqlSafe(format!(
            "UPDATE jobs SET state = 'failed', locked_until = NULL,
                 last_error = 'the worker stopped answering on the final attempt'
             WHERE id IN (
                 SELECT id FROM jobs
                 WHERE state = 'running' AND locked_until < now() AND attempts >= max_attempts
                 FOR UPDATE SKIP LOCKED)
             RETURNING {}",
            Self::PHOTO_ID
        )))
        .fetch_all(&mut *transaction)
        .await?;

        for photo_id in &expired_photos {
            Self::mark_linked_photo_failed_in(&mut transaction, photo_id.as_deref()).await?;
        }

        transaction.commit().await?;

        Ok(sqlx::query_as(AssertSqlSafe(format!(
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
        )))
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

    /// Deletes a finished job. The completion tells whether the claim was still current.
    pub async fn complete(&self, job: &Job) -> Result<JobCompletion, DatabaseError> {
        Self::complete_in(self.pool, job).await
    }

    /// Finishes `job` like [`complete`](Self::complete) and records the success, so
    /// [`ran_within`](Self::ran_within) can tell a later run that it came too soon. Nothing is
    /// recorded when the claim is no longer current.
    pub async fn complete_recorded(&self, job: &Job) -> Result<JobCompletion, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let completion = Self::complete_in(&mut *transaction, job).await?;

        if completion.is_current() {
            sqlx::query(
                "INSERT INTO job_runs(kind, finished_at) VALUES ($1, now())
                 ON CONFLICT (kind) DO UPDATE SET finished_at = now()",
            )
            .bind(job.kind().as_str())
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;

        Ok(completion)
    }

    /// Whether a job of `kind` was recorded as completed less than `window` ago.
    pub async fn ran_within(&self, kind: JobKind, window: Duration) -> Result<bool, DatabaseError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(
                 SELECT 1 FROM job_runs
                 WHERE kind = $1 AND finished_at > now() - $2 * interval '1 second')",
        )
        .bind(kind.as_str())
        .bind(window.as_secs_f64())
        .fetch_one(self.pool)
        .await?)
    }

    /// Records a photo job's result and finishes the job in one transaction, so a crash can
    /// never leave one without the other.
    ///
    /// Nothing is saved when the claim is no longer current: the run that owns the job now
    /// records its own result. Otherwise the photo changes only if it still exists and is
    /// unfinished. Either way, the returned [`PhotoUpdate`] tells whether the photo still
    /// exists, so late work can remove what it wrote for a deleted one.
    pub async fn complete_photo(
        &self,
        job: &Job,
        photo_id: PhotoId,
        result: PhotoResult<'_>,
    ) -> Result<(JobCompletion, PhotoUpdate), DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let completion = Self::complete_in(&mut *transaction, job).await?;
        let saved = match (completion.is_current(), result) {
            (false, _) => false,
            (true, PhotoResult::Ready) => {
                PhotoRepository::mark_ready_in(&mut transaction, photo_id).await?
            }
            (true, PhotoResult::Failed(reason)) => {
                PhotoRepository::mark_failed_in(&mut transaction, photo_id, reason).await?
            }
        };
        let update = if saved {
            PhotoUpdate::Saved
        } else if PhotoRepository::exists_in(&mut transaction, photo_id).await? {
            PhotoUpdate::Unchanged
        } else {
            PhotoUpdate::Missing
        };

        transaction.commit().await?;

        Ok((completion, update))
    }

    /// Records a recoverable failure: the job is requeued after `backoff`, or becomes
    /// [`Failed`](JobState::Failed) when this was its last attempt, which also fails its photo
    /// with [`PROCESSING_FAILED_REASON`].
    ///
    /// Returns the new state, or `None` if the claim was no longer current.
    pub async fn fail(
        &self,
        job: &Job,
        error: &str,
        backoff: Backoff,
    ) -> Result<Option<JobState>, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let failed: Option<(JobState, Option<String>)> = sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE jobs SET
                 state = CASE WHEN attempts >= max_attempts
                     THEN 'failed'::job_state ELSE 'queued'::job_state END,
                 run_at = CASE WHEN attempts >= max_attempts
                     THEN run_at ELSE now() + $4 * interval '1 second' END,
                 locked_until = NULL,
                 last_error = $3
             WHERE id = $1 AND state = 'running' AND attempts = $2
             RETURNING state, {}",
            Self::PHOTO_ID
        )))
        .bind(job.id)
        .bind(attempts_param(job))
        .bind(clean_error(error))
        .bind(backoff.delay(job.attempts).as_secs_f64())
        .fetch_optional(&mut *transaction)
        .await?;

        let Some((state, photo_id)) = failed else {
            return Ok(None);
        };

        if state == JobState::Failed {
            Self::mark_linked_photo_failed_in(&mut transaction, photo_id.as_deref()).await?;
        }

        transaction.commit().await?;

        Ok(Some(state))
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

    /// Finishes `job` if this run still owns it, through `executor`.
    async fn complete_in(
        executor: impl PgExecutor<'_>,
        job: &Job,
    ) -> Result<JobCompletion, DatabaseError> {
        let done =
            sqlx::query("DELETE FROM jobs WHERE id = $1 AND state = 'running' AND attempts = $2")
                .bind(job.id)
                .bind(attempts_param(job))
                .execute(executor)
                .await?;

        Ok(JobCompletion {
            current: done.rows_affected() == 1,
        })
    }

    /// Marks the photo of a job that failed for good, so the gallery shows why it has no proof.
    ///
    /// Jobs without a readable photo id have nothing to update.
    async fn mark_linked_photo_failed_in(
        transaction: &mut Transaction<'_, Postgres>,
        raw_photo_id: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let Some(photo_id) = raw_photo_id.and_then(|id| Uuid::parse_str(id).ok()) else {
            return Ok(());
        };

        PhotoRepository::mark_failed_in(
            transaction,
            PhotoId::from(photo_id),
            PROCESSING_FAILED_REASON,
        )
        .await?;

        Ok(())
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
    use std::{collections::HashSet, num::NonZeroU16, time::Duration};

    use serde_json::json;
    use tokio::{task::JoinSet, time};

    use super::*;
    use crate::{
        Database,
        auth::WorkspaceId,
        gallery::GalleryRepository,
        photo::{
            NewPhoto, PROCESSING_FAILED_REASON, PhotoDimensions, PhotoKind, PhotoRepository,
            PhotoStatus,
        },
    };

    const LEASE: Duration = Duration::from_millis(300);
    const BACKOFF: Backoff = Backoff {
        base: Duration::from_millis(100),
        cap: Duration::from_millis(400),
    };
    const KINDS: &[JobKind] = &[JobKind::ProcessPhoto];

    #[sqlx::test(migrations = "./migrations")]
    async fn enqueue_in_obeys_the_callers_transaction(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();

        JobRepository::enqueue_in(
            &mut transaction,
            process_photo(),
            NonZeroU16::new(3).unwrap(),
        )
        .await
        .unwrap();
        let count_in_transaction: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        assert_eq!(count_in_transaction, 1);
        transaction.rollback().await.unwrap();

        let count_after_rollback: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(count_after_rollback, 0);
    }

    fn process_photo() -> JobPayload {
        JobPayload::ProcessPhoto(ProcessPhoto {
            photo_id: PhotoId::from(Uuid::new_v4()),
        })
    }

    async fn enqueue(database: &Database, max_attempts: u16) -> JobId {
        database
            .repository::<JobRepository>()
            .enqueue(process_photo(), NonZeroU16::new(max_attempts).unwrap())
            .await
            .unwrap()
    }

    /// Inserts a photo and its processing job, capped at `max_attempts`.
    async fn photo_job(database: &Database, pool: &PgPool, max_attempts: u16) -> PhotoId {
        let workspace_id = WorkspaceId(Uuid::new_v4());

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(pool)
            .await
            .unwrap();

        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photo_id = PhotoId::generate();
        let photo = NewPhoto {
            kind: PhotoKind::Proof,
            id: photo_id,
            workspace_id,
            gallery_id: gallery.id,
            original_name: "proof.jpg".to_owned(),
            size: 1024.try_into().unwrap(),
            dimensions: PhotoDimensions {
                width: 20.try_into().unwrap(),
                height: 10.try_into().unwrap(),
            },
        };
        let mut transaction = pool.begin().await.unwrap();

        database
            .repository::<PhotoRepository>()
            .insert_in(&mut transaction, &photo)
            .await
            .unwrap();
        JobRepository::enqueue_in(
            &mut transaction,
            JobPayload::ProcessPhoto(ProcessPhoto { photo_id }),
            NonZeroU16::new(max_attempts).unwrap(),
        )
        .await
        .unwrap();
        transaction.commit().await.unwrap();

        photo_id
    }

    async fn photo_status(database: &Database, photo_id: PhotoId) -> PhotoStatus {
        let photos = database.repository::<PhotoRepository>();
        let location = photos.locate(photo_id).await.unwrap().unwrap();

        photos
            .get(location.workspace_id, location.gallery_id, photo_id)
            .await
            .unwrap()
            .unwrap()
            .status
    }

    #[test]
    fn backoff_doubles_up_to_the_cap() {
        let backoff = Backoff::new(Duration::from_secs(10), Duration::from_secs(60));
        let delays: Vec<_> = (1..=5).map(|n| backoff.delay(n).as_secs()).collect();

        assert_eq!(delays, [10, 20, 40, 60, 60]);
        assert_eq!(backoff.delay(u32::MAX), Duration::from_secs(60));
    }

    #[test]
    fn process_photo_payload_keeps_the_stored_shape() {
        let photo_id = PhotoId::from(Uuid::new_v4());
        let payload = JobPayload::ProcessPhoto(ProcessPhoto { photo_id });
        let stored =
            json!({"kind": "process_photo", "payload": {"photo_id": photo_id.to_string()}});

        assert_eq!(serde_json::to_value(&payload).unwrap(), stored);
        assert_eq!(
            serde_json::from_value::<JobPayload>(stored).unwrap(),
            payload
        );
    }

    #[test]
    fn job_kind_parses_its_tag() {
        assert_eq!(
            "process_photo".parse::<JobKind>().unwrap(),
            JobKind::ProcessPhoto
        );
        assert!("unknown".parse::<JobKind>().is_err());
    }

    #[test]
    fn cleanup_payloads_keep_their_tag_and_ids() {
        let workspace_id = WorkspaceId(Uuid::new_v4());
        let gallery_id = GalleryId(Uuid::new_v4());
        let photo_id = PhotoId::from(Uuid::new_v4());
        let cases = [
            (
                JobPayload::CleanupGallery(CleanupGallery {
                    workspace_id,
                    gallery_id,
                }),
                JobKind::CleanupGallery,
                json!({"kind": "cleanup_gallery", "payload": {
                    "workspace_id": workspace_id.0, "gallery_id": gallery_id.0}}),
            ),
            (
                JobPayload::CleanupPhotos(CleanupPhotos {
                    workspace_id,
                    gallery_id,
                    photo_ids: vec![photo_id],
                }),
                JobKind::CleanupPhotos,
                json!({"kind": "cleanup_photos", "payload": {
                    "workspace_id": workspace_id.0, "gallery_id": gallery_id.0,
                    "photo_ids": [photo_id.to_string()]}}),
            ),
            (
                JobPayload::SweepOrphans,
                JobKind::SweepOrphans,
                json!({"kind": "sweep_orphans"}),
            ),
        ];

        for (payload, kind, stored) in cases {
            assert_eq!(payload.kind(), kind);
            assert_eq!(serde_json::to_value(&payload).unwrap(), stored);
            assert_eq!(
                serde_json::from_value::<JobPayload>(stored).unwrap(),
                payload
            );
            assert_eq!(kind.as_str().parse::<JobKind>().unwrap(), kind);
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_recorded_completion_counts_as_a_recent_run(pool: PgPool) {
        let database = Database::new(pool);
        let jobs = database.repository::<JobRepository>();
        let window = Duration::from_secs(3600);

        assert!(
            !jobs
                .ran_within(JobKind::SweepOrphans, window)
                .await
                .unwrap()
        );

        jobs.enqueue(JobPayload::SweepOrphans, NonZeroU16::new(3).unwrap())
            .await
            .unwrap();

        let sweep = jobs
            .claim(&[JobKind::SweepOrphans], Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();

        assert!(jobs.complete_recorded(&sweep).await.unwrap().is_current());
        assert!(
            jobs.ran_within(JobKind::SweepOrphans, window)
                .await
                .unwrap()
        );
        assert!(
            !jobs
                .ran_within(JobKind::SweepOrphans, Duration::ZERO)
                .await
                .unwrap()
        );
        assert!(
            !jobs
                .ran_within(JobKind::CleanupGallery, window)
                .await
                .unwrap()
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
        assert!(!jobs.complete(&first).await.unwrap().is_current());
        assert!(jobs.complete(&second).await.unwrap().is_current());
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
        let payload = process_photo();
        jobs.enqueue(payload.clone(), NonZeroU16::new(3).unwrap())
            .await
            .unwrap();

        assert!(jobs.claim(&[], LEASE).await.unwrap().is_none());
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        assert_eq!(job.kind(), JobKind::ProcessPhoto);
        assert_eq!(
            JobPayload::ProcessPhoto(job.payload::<ProcessPhoto>().unwrap()),
            payload
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn payload_migration_preserves_existing_job_data(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("job_payload_migration_{}", Uuid::new_v4().simple());
        sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("SET LOCAL search_path TO {schema}")))
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

    #[sqlx::test(migrations = "./migrations")]
    async fn exhausted_job_fails_its_photo(pool: PgPool) {
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();
        let photo_id = photo_job(&database, &pool, 1).await;
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        assert_eq!(
            jobs.fail(&job, "decoder exploded", BACKOFF).await.unwrap(),
            Some(JobState::Failed)
        );
        assert_eq!(
            photo_status(&database, photo_id).await,
            PhotoStatus::Failed(PROCESSING_FAILED_REASON.to_owned())
        );
        assert_eq!(
            jobs.list_failed().await.unwrap()[0].last_error.as_deref(),
            Some("decoder exploded")
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retryable_failure_leaves_the_photo_alone(pool: PgPool) {
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();
        let photo_id = photo_job(&database, &pool, 2).await;
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        assert_eq!(
            jobs.fail(&job, "boom", BACKOFF).await.unwrap(),
            Some(JobState::Queued)
        );
        assert_eq!(
            photo_status(&database, photo_id).await,
            PhotoStatus::Waiting
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn crash_looping_job_fails_its_photo(pool: PgPool) {
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();
        let photo_id = photo_job(&database, &pool, 1).await;

        jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        time::sleep(LEASE + Duration::from_millis(100)).await;

        assert!(jobs.claim(KINDS, LEASE).await.unwrap().is_none());
        assert_eq!(
            photo_status(&database, photo_id).await,
            PhotoStatus::Failed(PROCESSING_FAILED_REASON.to_owned())
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn late_failure_does_not_touch_a_ready_photo(pool: PgPool) {
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();
        let photo_id = photo_job(&database, &pool, 1).await;
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        assert!(
            database
                .repository::<PhotoRepository>()
                .mark_ready(photo_id)
                .await
                .unwrap()
        );
        assert_eq!(
            jobs.fail(&job, "too late", BACKOFF).await.unwrap(),
            Some(JobState::Failed)
        );
        assert_eq!(photo_status(&database, photo_id).await, PhotoStatus::Ready);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn complete_photo_records_the_result_and_finishes_the_job_together(pool: PgPool) {
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();

        for (result, expected) in [
            (PhotoResult::Ready, PhotoStatus::Ready),
            (
                PhotoResult::Failed("unreadable"),
                PhotoStatus::Failed("unreadable".to_owned()),
            ),
        ] {
            let photo_id = photo_job(&database, &pool, 3).await;
            let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
            let (completion, update) = jobs.complete_photo(&job, photo_id, result).await.unwrap();

            assert!(completion.is_current());
            assert_eq!(update, PhotoUpdate::Saved);
            assert_eq!(photo_status(&database, photo_id).await, expected);
            assert!(jobs.list_failed().await.unwrap().is_empty());
            assert!(jobs.claim(KINDS, LEASE).await.unwrap().is_none());
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn complete_photo_with_a_lost_claim_changes_nothing(pool: PgPool) {
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();
        let photo_id = photo_job(&database, &pool, 3).await;
        let stale = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        time::sleep(LEASE + Duration::from_millis(100)).await;
        let current = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        let (completion, update) = jobs
            .complete_photo(&stale, photo_id, PhotoResult::Failed("unreadable"))
            .await
            .unwrap();

        assert!(!completion.is_current());
        assert_eq!(update, PhotoUpdate::Unchanged);
        assert_eq!(
            photo_status(&database, photo_id).await,
            PhotoStatus::Waiting
        );
        assert!(jobs.complete(&current).await.unwrap().is_current());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn complete_photo_reports_a_deleted_photo_whether_or_not_the_claim_is_current(
        pool: PgPool,
    ) {
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();
        let photo_id = photo_job(&database, &pool, 3).await;
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(photo_id)
            .execute(&pool)
            .await
            .unwrap();

        let (_, update) = jobs
            .complete_photo(&job, photo_id, PhotoResult::Ready)
            .await
            .unwrap();

        assert_eq!(update, PhotoUpdate::Missing);

        // The job is finished now, so the same claim is no longer current.
        let (completion, update) = jobs
            .complete_photo(&job, photo_id, PhotoResult::Ready)
            .await
            .unwrap();

        assert!(!completion.is_current());
        assert_eq!(update, PhotoUpdate::Missing);
    }
}
