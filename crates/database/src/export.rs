//! Photographer ZIP exports of a gallery's selection.
//!
//! An export is requested once per click: [`ExportRepository::request`] stores the
//! favorites as they are at that moment, in one transaction, together with the job that builds
//! the ZIP. The snapshot has no foreign key to `photos`, so favorite changes and deleted photos
//! never rewrite it; the build notices a photo that is gone and the export then fails instead
//! of producing a smaller archive.
//!
//! At most one export per gallery is being prepared, and a new request replaces the previous
//! finished one. An export older than the retention is treated as gone. A status change is
//! announced on [`ExportEvent`]'s channel.

use std::{num::NonZeroU64, time::Duration};

use pp_core::types::{generate::Generate, timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{
    AssertSqlSafe, FromRow, PgPool, Postgres, Row, Transaction, Type, postgres::PgRow, types::Json,
};
use uuid::Uuid;

use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::GalleryId,
    jobs::{Job, JobCompletion, JobOptions, JobRepository, ZipExport},
    listener::Channel,
    photo::PhotoId,
};

/// Reason shown when the worker gives up on an export after its last attempt.
pub const EXPORT_FAILED_REASON: &str = "The ZIP could not be prepared after several attempts.";

/// Reason shown when a photo of the selection was deleted before its file was added.
pub const PHOTO_REMOVED_REASON: &str = "A selected photo was removed. Try again.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct ExportId(pub Uuid);

impl Generate for ExportId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

/// An export's status changed. It carries no status: whoever shows it refetches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportEvent {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
}

impl Channel for ExportEvent {
    const NAME: &'static str = "export_events";
}

/// Where an export stands. Stored and serialized as `{"kind": ..}`, with the `reason` next to
/// the kind when it failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "reason", rename_all = "snake_case")]
pub enum ExportStatus {
    Preparing,
    Ready,
    /// The reason is safe to show.
    Failed(String),
}

/// One export as the photographer sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Export {
    pub id: ExportId,
    pub status: ExportStatus,
    pub photo_count: u32,
    pub created_at: Timestamp,
}

impl FromRow<'_, PgRow> for Export {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let photo_count = u32::try_from(row.try_get::<i32, _>("photo_count")?)
            .map_err(|_| sqlx::Error::Decode("photo count is negative".into()))?;

        Ok(Self {
            id: row.try_get("id")?,
            status: row.try_get::<Json<ExportStatus>, _>("status")?.0,
            photo_count,
            created_at: row.try_get("created_at")?,
        })
    }
}

/// What [`ExportRepository::request`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportRequest {
    /// A new snapshot was stored and its job queued. `replaced` are the previous exports of the
    /// gallery, whose files are no longer needed.
    Created {
        export: Export,
        replaced: Vec<ExportId>,
    },
    /// The gallery already has an export being prepared; it is returned instead.
    InProgress(Export),
    EmptySelection,
    InvalidSelection,
    /// The originals together exceed the allowed size.
    TooLarge,
    MissingGallery,
}

/// A photo of the snapshot, in archive order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPhoto {
    pub id: PhotoId,
    pub original_name: String,
}

/// What the worker needs to build an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlan {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
    pub photos: Vec<ExportPhoto>,
}

/// What [`ExportRepository::complete`] did to the export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportUpdate {
    /// The export is ready.
    Saved,
    /// A selected photo no longer exists, so the export failed instead.
    PhotoRemoved,
    /// The export no longer exists or already finished: the file is not wanted.
    Gone,
}

/// A file location of an expired export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct ExpiredExport {
    pub id: ExportId,
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
}

pub struct ExportRepository<'db> {
    pool: &'db PgPool,
    options: JobOptions,
}

impl<'db> Repository<'db> for ExportRepository<'db> {
    type Options = JobOptions;

    fn open(pool: &'db PgPool, options: Self::Options) -> Self {
        Self { pool, options }
    }
}

impl ExportRepository<'_> {
    const COLUMNS: &'static str = "id, status, photo_count, created_at";
    const EXPORT_JOBS: &'static str = "payload->>'kind' = 'zip_export'
        AND payload->'payload'->>'export_id'";

    /// Stores the gallery's favorites as a new export and queues its build, all in one
    /// transaction. Concurrent requests for one gallery are serialized on its row.
    pub async fn request(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        max_size: NonZeroU64,
    ) -> Result<ExportRequest, DatabaseError> {
        self.request_selected(workspace_id, gallery_id, max_size, None)
            .await
    }

    /// Creates a ZIP snapshot of either every favorite or the explicitly selected favorite proofs.
    /// Unknown, removed or nonfavorite photos invalidate the whole subset.
    pub async fn request_selected(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        max_size: NonZeroU64,
        selected: Option<&[PhotoId]>,
    ) -> Result<ExportRequest, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let gallery: Option<GalleryId> = sqlx::query_scalar(
            "SELECT id FROM galleries WHERE id = $1 AND workspace_id = $2 FOR NO KEY UPDATE",
        )
        .bind(gallery_id)
        .bind(workspace_id)
        .fetch_optional(&mut *transaction)
        .await?;

        if gallery.is_none() {
            return Ok(ExportRequest::MissingGallery);
        }

        let preparing: Option<Export> = sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM exports WHERE gallery_id = $1 AND status ->> 'kind' = 'preparing'",
            Self::COLUMNS
        )))
        .bind(gallery_id)
        .fetch_optional(&mut *transaction)
        .await?;

        if let Some(export) = preparing {
            return Ok(ExportRequest::InProgress(export));
        }

        let favorites: Vec<(PhotoId, String, i64)> = sqlx::query_as(
            "SELECT id, original_name, size FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND favorite AND kind = 'proof'
               AND removed_at IS NULL AND ($3::uuid[] IS NULL OR id = ANY($3))
             ORDER BY created_at, id",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(selected.map(|ids| ids.to_vec()))
        .fetch_all(&mut *transaction)
        .await?;

        if selected.is_some_and(|ids| ids.is_empty() || ids.len() != favorites.len()) {
            return Ok(ExportRequest::InvalidSelection);
        }

        if favorites.is_empty() {
            return Ok(ExportRequest::EmptySelection);
        }

        let total: i64 = favorites.iter().map(|(_, _, size)| size).sum();

        if u64::try_from(total).map_or(true, |total| total > max_size.get()) {
            return Ok(ExportRequest::TooLarge);
        }

        let replaced_ids = Self::delete_exports_in(&mut transaction, gallery_id).await?;
        let id = ExportId::generate();
        let photo_count = i32::try_from(favorites.len())
            .map_err(|_| sqlx::Error::Encode("selection is too large to count".into()))?;
        let export: Export = sqlx::query_as(AssertSqlSafe(format!(
            "INSERT INTO exports (id, workspace_id, gallery_id, photo_count, size)
             VALUES ($1, $2, $3, $4, $5) RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(photo_count)
        .bind(total)
        .fetch_one(&mut *transaction)
        .await?;

        let ids: Vec<PhotoId> = favorites.iter().map(|(id, ..)| *id).collect();
        let names: Vec<&str> = favorites.iter().map(|(_, name, _)| name.as_str()).collect();

        sqlx::query(
            "INSERT INTO export_photos (export_id, photo_id, original_name, position)
             SELECT $1, photo_id, original_name, position::integer
             FROM unnest($2::uuid[], $3::text[]) WITH ORDINALITY AS t(photo_id, original_name, position)",
        )
        .bind(id)
        .bind(&ids)
        .bind(&names)
        .execute(&mut *transaction)
        .await?;
        JobRepository::enqueue_in(
            &mut transaction,
            ZipExport { export_id: id },
            self.options.max_attempts,
        )
        .await?;
        transaction.commit().await?;

        Ok(ExportRequest::Created {
            export,
            replaced: replaced_ids,
        })
    }

    /// The gallery's newest export that has not outlived `retention`.
    pub async fn latest(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        retention: Duration,
    ) -> Result<Option<Export>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM exports
             WHERE workspace_id = $1 AND gallery_id = $2
               AND COALESCE(finished_at, created_at) > now() - $3 * interval '1 second'
             ORDER BY created_at DESC LIMIT 1",
            Self::COLUMNS
        )))
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(retention.as_secs_f64())
        .fetch_optional(self.pool)
        .await?)
    }

    /// The export `id` when it belongs to this workspace and gallery, is ready and has not
    /// outlived `retention`. Anything else is indistinguishable from a missing one.
    pub async fn ready(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        id: ExportId,
        retention: Duration,
    ) -> Result<Option<Export>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM exports
             WHERE id = $1 AND workspace_id = $2 AND gallery_id = $3 AND status ->> 'kind' = 'ready'
               AND finished_at > now() - $4 * interval '1 second'",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(retention.as_secs_f64())
        .fetch_optional(self.pool)
        .await?)
    }

    /// The snapshot of an export still being prepared; `None` once it finished or was deleted.
    pub async fn plan(&self, id: ExportId) -> Result<Option<ExportPlan>, DatabaseError> {
        let export: Option<(WorkspaceId, GalleryId)> = sqlx::query_as(
            "SELECT workspace_id, gallery_id FROM exports
             WHERE id = $1 AND status ->> 'kind' = 'preparing'",
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await?;
        let Some((workspace_id, gallery_id)) = export else {
            return Ok(None);
        };
        let photos = sqlx::query_as::<_, (PhotoId, String)>(
            "SELECT photo_id, original_name FROM export_photos
             WHERE export_id = $1 ORDER BY position",
        )
        .bind(id)
        .fetch_all(self.pool)
        .await?
        .into_iter()
        .map(|(id, original_name)| ExportPhoto { id, original_name })
        .collect();

        Ok(Some(ExportPlan {
            workspace_id,
            gallery_id,
            photos,
        }))
    }

    /// Whether the export still has a row, so its file is wanted.
    pub async fn exists(&self, id: ExportId) -> Result<bool, DatabaseError> {
        Ok(
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM exports WHERE id = $1)")
                .bind(id)
                .fetch_one(self.pool)
                .await?,
        )
    }

    /// Records the finished file and completes the job in one transaction.
    ///
    /// Nothing changes when the claim is no longer current. The export becomes ready only if
    /// every photo of its snapshot still exists; otherwise it fails with
    /// [`PHOTO_REMOVED_REASON`], never leaving a ready export that may be missing a photo.
    pub async fn complete(
        &self,
        job: &Job,
        id: ExportId,
    ) -> Result<(JobCompletion, ExportUpdate), DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let completion = JobRepository::complete_in(&mut *transaction, job).await?;

        if !completion.is_current() {
            return Ok((completion, ExportUpdate::Gone));
        }

        let marked_ready = sqlx::query(
            "UPDATE exports SET status = $2, finished_at = now()
             WHERE id = $1 AND status ->> 'kind' = 'preparing'
               AND NOT EXISTS (
                   SELECT 1 FROM export_photos s
                   WHERE s.export_id = $1
                     AND NOT EXISTS (SELECT 1 FROM photos p WHERE p.id = s.photo_id))",
        )
        .bind(id)
        .bind(Json(ExportStatus::Ready))
        .execute(&mut *transaction)
        .await?;
        let update = if marked_ready.rows_affected() == 1 {
            ExportUpdate::Saved
        } else if Self::mark_failed_in(&mut transaction, id, PHOTO_REMOVED_REASON).await? {
            ExportUpdate::PhotoRemoved
        } else {
            ExportUpdate::Gone
        };

        transaction.commit().await?;

        Ok((completion, update))
    }

    /// Fails the export with `reason` and completes the job, for errors a retry cannot fix.
    pub async fn fail(
        &self,
        job: &Job,
        id: ExportId,
        reason: &str,
    ) -> Result<JobCompletion, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let completion = JobRepository::complete_in(&mut *transaction, job).await?;

        if completion.is_current() {
            Self::mark_failed_in(&mut transaction, id, reason).await?;
        }

        transaction.commit().await?;

        Ok(completion)
    }

    /// Fails an export still being prepared; `false` if it already finished or is gone.
    pub(crate) async fn mark_failed_in(
        transaction: &mut Transaction<'_, Postgres>,
        id: ExportId,
        reason: &str,
    ) -> Result<bool, DatabaseError> {
        let failed = sqlx::query(
            "UPDATE exports SET status = $2, finished_at = now()
             WHERE id = $1 AND status ->> 'kind' = 'preparing'",
        )
        .bind(id)
        .bind(Json(ExportStatus::Failed(reason.to_owned())))
        .execute(&mut **transaction)
        .await?;

        Ok(failed.rows_affected() == 1)
    }

    /// Deletes the exports that have outlived `retention`, with their jobs, and returns them so
    /// their files can follow.
    pub async fn expire(&self, retention: Duration) -> Result<Vec<ExpiredExport>, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let expired: Vec<ExpiredExport> = sqlx::query_as(
            "SELECT id, workspace_id, gallery_id FROM exports
             WHERE COALESCE(finished_at, created_at) <= now() - $1 * interval '1 second'
             ORDER BY id FOR UPDATE",
        )
        .bind(retention.as_secs_f64())
        .fetch_all(&mut *transaction)
        .await?;
        let ids: Vec<ExportId> = expired.iter().map(|export| export.id).collect();

        Self::delete_jobs_in(&mut transaction, &ids).await?;
        sqlx::query("DELETE FROM exports WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;

        Ok(expired)
    }

    /// Deletes the gallery's exports and their jobs, jobs first like every other deletion.
    pub(crate) async fn delete_exports_in(
        transaction: &mut Transaction<'_, Postgres>,
        gallery_id: GalleryId,
    ) -> Result<Vec<ExportId>, DatabaseError> {
        let ids: Vec<ExportId> = sqlx::query_scalar(
            "SELECT id FROM exports WHERE gallery_id = $1 ORDER BY id FOR UPDATE",
        )
        .bind(gallery_id)
        .fetch_all(&mut **transaction)
        .await?;

        Self::delete_jobs_in(transaction, &ids).await?;
        sqlx::query("DELETE FROM exports WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut **transaction)
            .await?;

        Ok(ids)
    }

    /// Deletes the build jobs of the exports, in id order so overlapping deletions do not deadlock.
    pub(crate) async fn delete_jobs_in(
        transaction: &mut Transaction<'_, Postgres>,
        ids: &[ExportId],
    ) -> Result<(), DatabaseError> {
        let ids: Vec<String> = ids.iter().map(|id| id.0.to_string()).collect();

        sqlx::query(AssertSqlSafe(format!(
            "DELETE FROM jobs WHERE id IN (
                 SELECT id FROM jobs WHERE {} = ANY($1) ORDER BY id FOR UPDATE
             )",
            Self::EXPORT_JOBS
        )))
        .bind(ids)
        .execute(&mut **transaction)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use sqlx::PgPool;

    use super::*;
    use crate::{
        Database,
        gallery::GalleryRepository,
        jobs::{Backoff, JobKind, JobState},
        tests::TestRepository,
        workspace_deletion::WorkspaceDeletion,
    };

    const MAX: NonZeroU64 = NonZeroU64::new(1 << 30).unwrap();
    const DAY: Duration = Duration::from_secs(86_400);

    struct Fixture {
        pool: PgPool,
        database: Database,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
    }

    impl Fixture {
        async fn new(pool: &PgPool) -> Self {
            let database = Database::new(pool.clone());
            let workspace_id = database
                .repository::<TestRepository>()
                .create_workspace()
                .await
                .unwrap();
            let gallery_id = database
                .repository::<GalleryRepository>()
                .create(workspace_id)
                .await
                .unwrap()
                .id;

            Self {
                pool: pool.clone(),
                database,
                workspace_id,
                gallery_id,
            }
        }

        async fn photo(&self, name: &str, size: i64, favorite: bool) -> PhotoId {
            let id = PhotoId::from(Uuid::new_v4());

            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width,
                                     height, status, favorite)
                 VALUES ($1, $2, $3, $4, $5, 2, 2, 'ready', $6)",
            )
            .bind(id)
            .bind(self.workspace_id)
            .bind(self.gallery_id)
            .bind(name)
            .bind(size)
            .bind(favorite)
            .execute(&self.pool)
            .await
            .unwrap();

            id
        }

        async fn request(&self) -> ExportRequest {
            self.exports()
                .request(self.workspace_id, self.gallery_id, MAX)
                .await
                .unwrap()
        }

        fn exports(&self) -> ExportRepository<'_> {
            self.database.repository::<ExportRepository>()
        }

        async fn created(&self) -> Export {
            match self.request().await {
                ExportRequest::Created { export, .. } => export,
                other => panic!("expected a new export, got {other:?}"),
            }
        }

        async fn claim(&self) -> Job {
            self.database
                .repository::<JobRepository>()
                .claim(&[JobKind::ZipExport], Duration::from_secs(60))
                .await
                .unwrap()
                .expect("a build job is due")
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_request_snapshots_the_favorites_and_queues_one_job(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let first = fixture.photo("a.jpg", 10, true).await;
        let second = fixture.photo("b.jpg", 20, true).await;

        fixture.photo("c.jpg", 30, false).await;

        let export = fixture.created().await;
        let plan = fixture.exports().plan(export.id).await.unwrap().unwrap();

        assert_eq!(export.status, ExportStatus::Preparing);
        assert_eq!(export.photo_count, 2);
        assert_eq!(
            plan.photos.iter().map(|photo| photo.id).collect::<Vec<_>>(),
            [first, second]
        );
        assert_eq!(
            fixture
                .database
                .repository::<TestRepository>()
                .count_jobs(Some(JobState::Queued))
                .await
                .unwrap(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn later_selection_changes_do_not_alter_the_snapshot(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let kept = fixture.photo("a.jpg", 10, true).await;
        let export = fixture.created().await;

        sqlx::query("UPDATE photos SET favorite = false")
            .execute(&pool)
            .await
            .unwrap();
        fixture.photo("late.jpg", 10, true).await;

        let plan = fixture.exports().plan(export.id).await.unwrap().unwrap();

        assert_eq!(plan.photos.len(), 1);
        assert_eq!(plan.photos[0].id, kept);
        assert_eq!(plan.photos[0].original_name, "a.jpg");
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_request_while_preparing_returns_the_running_export(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;

        let export = fixture.created().await;

        assert_eq!(fixture.request().await, ExportRequest::InProgress(export));
        assert_eq!(
            fixture
                .database
                .repository::<TestRepository>()
                .count_jobs(None)
                .await
                .unwrap(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn empty_oversized_and_unknown_requests_create_nothing(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        assert_eq!(fixture.request().await, ExportRequest::EmptySelection);

        fixture.photo("a.jpg", 10, true).await;
        fixture.photo("b.jpg", 10, true).await;

        let outcome = fixture
            .exports()
            .request(
                fixture.workspace_id,
                fixture.gallery_id,
                NonZeroU64::new(19).unwrap(),
            )
            .await
            .unwrap();
        let other_workspace = fixture
            .exports()
            .request(WorkspaceId::generate(), fixture.gallery_id, MAX)
            .await
            .unwrap();

        assert_eq!(outcome, ExportRequest::TooLarge);
        assert_eq!(other_workspace, ExportRequest::MissingGallery);
        assert_eq!(
            fixture
                .database
                .repository::<TestRepository>()
                .count_jobs(None)
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_new_request_replaces_the_finished_export_and_its_job(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;

        let old = fixture.created().await;
        let job = fixture.claim().await;

        let _ = fixture.exports().complete(&job, old.id).await.unwrap();

        let ExportRequest::Created { export, replaced } = fixture.request().await else {
            panic!("expected a new export");
        };

        assert_eq!(replaced, [old.id]);
        assert_ne!(export.id, old.id);
        assert!(!fixture.exports().exists(old.id).await.unwrap());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn completing_marks_ready_only_while_every_photo_exists(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let photo = fixture.photo("a.jpg", 10, true).await;
        let export = fixture.created().await;
        let job = fixture.claim().await;

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(photo)
            .execute(&pool)
            .await
            .unwrap();

        let (completion, update) = fixture.exports().complete(&job, export.id).await.unwrap();
        let latest = fixture
            .exports()
            .latest(fixture.workspace_id, fixture.gallery_id, DAY)
            .await
            .unwrap()
            .unwrap();

        assert!(completion.is_current());
        assert_eq!(update, ExportUpdate::PhotoRemoved);
        assert_eq!(
            latest.status,
            ExportStatus::Failed(PHOTO_REMOVED_REASON.to_owned())
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_complete_export_is_ready_and_downloadable_only_in_its_gallery(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;

        let export = fixture.created().await;
        let job = fixture.claim().await;
        let (_, update) = fixture.exports().complete(&job, export.id).await.unwrap();

        assert_eq!(update, ExportUpdate::Saved);
        assert!(
            fixture
                .exports()
                .ready(fixture.workspace_id, fixture.gallery_id, export.id, DAY)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            fixture
                .exports()
                .ready(WorkspaceId::generate(), fixture.gallery_id, export.id, DAY)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            fixture
                .exports()
                .ready(fixture.workspace_id, GalleryId::generate(), export.id, DAY)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            fixture
                .exports()
                .ready(
                    fixture.workspace_id,
                    fixture.gallery_id,
                    export.id,
                    Duration::ZERO
                )
                .await
                .unwrap()
                .is_none()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_failed_job_fails_the_export_with_its_reason(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;

        let export = fixture.created().await;
        let job = fixture.claim().await;

        let _ = fixture
            .exports()
            .fail(&job, export.id, "The server is out of disk space.")
            .await
            .unwrap();

        let latest = fixture
            .exports()
            .latest(fixture.workspace_id, fixture.gallery_id, DAY)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            latest.status,
            ExportStatus::Failed("The server is out of disk space.".to_owned())
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn exhausting_the_attempts_fails_the_export(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;
        let export = fixture.created().await;

        sqlx::query("UPDATE jobs SET max_attempts = 1")
            .execute(&pool)
            .await
            .unwrap();

        let job = fixture.claim().await;

        fixture
            .database
            .repository::<JobRepository>()
            .fail(
                &job,
                "disk error",
                Backoff::new(Duration::from_secs(1), DAY),
            )
            .await
            .unwrap();

        let latest = fixture
            .exports()
            .latest(fixture.workspace_id, fixture.gallery_id, DAY)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(export.id, latest.id);
        assert_eq!(
            latest.status,
            ExportStatus::Failed(EXPORT_FAILED_REASON.to_owned())
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn an_expired_last_attempt_lease_fails_the_export(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;
        fixture.created().await;
        sqlx::query("UPDATE jobs SET max_attempts = 1")
            .execute(&pool)
            .await
            .unwrap();

        let jobs = fixture.database.repository::<JobRepository>();

        jobs.claim(&[JobKind::ZipExport], Duration::ZERO)
            .await
            .unwrap()
            .unwrap();

        assert!(
            jobs.claim(&[JobKind::ZipExport], Duration::from_secs(60))
                .await
                .unwrap()
                .is_none()
        );

        let latest = fixture
            .exports()
            .latest(fixture.workspace_id, fixture.gallery_id, DAY)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            latest.status,
            ExportStatus::Failed(EXPORT_FAILED_REASON.to_owned())
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn database_accepts_only_well_formed_statuses(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;

        let export = fixture.created().await;

        for (status, finished) in [
            (r#"{"kind": "failed"}"#, true),
            (r#"{"kind": "failed", "reason": 3}"#, true),
            (r#"{"kind": "ready", "reason": "no"}"#, true),
            (r#"{"kind": "paused"}"#, true),
            (r#"{"kind": "ready"}"#, false),
            (r#"{"kind": "preparing"}"#, true),
        ] {
            let outcome = sqlx::query(
                "UPDATE exports SET status = $2::jsonb,
                     finished_at = CASE WHEN $3 THEN now() END WHERE id = $1",
            )
            .bind(export.id)
            .bind(status)
            .bind(finished)
            .execute(&pool)
            .await;

            assert!(outcome.is_err(), "{status} was accepted");
        }

        sqlx::query(
            r#"UPDATE exports SET status = '{"kind": "failed", "reason": "r"}', finished_at = now()"#,
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn expiring_deletes_old_exports_with_their_jobs(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;

        let export = fixture.created().await;

        assert!(fixture.exports().expire(DAY).await.unwrap().is_empty());

        let expired = fixture.exports().expire(Duration::ZERO).await.unwrap();

        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, export.id);
        assert!(!fixture.exports().exists(export.id).await.unwrap());
        assert_eq!(
            fixture
                .database
                .repository::<TestRepository>()
                .count_jobs(None)
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_the_gallery_removes_its_export_and_build_job(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;

        fixture.photo("a.jpg", 10, true).await;

        let export = fixture.created().await;

        assert!(
            fixture
                .database
                .repository::<WorkspaceDeletion>()
                .delete_gallery(fixture.workspace_id, fixture.gallery_id)
                .await
                .unwrap()
        );
        assert!(!fixture.exports().exists(export.id).await.unwrap());
        assert!(
            fixture
                .database
                .repository::<TestRepository>()
                .jobs_of_kind("zip_export")
                .await
                .unwrap()
                .is_empty()
        );
    }
}
