//! Builds the ZIP of a photographer's selection from its stored snapshot.

use std::{sync::Arc, time::Duration};

use pp_database::{
    Database,
    export::{ExportId, ExportPlan, ExportRepository, ExportUpdate, PHOTO_REMOVED_REASON},
    jobs::{Job, JobCompletion, JobRepository, ZipExport},
    photo::PhotoLocation,
};
use pp_storage::{ArchiveEntry, ArchiveKey, PhotoKey, PhotoStorage, StorageError, Variant};
use tracing::{info, warn};

use crate::handlers::HandlerError;

/// Reason shown when the photo volume is full; retrying cannot help until space is freed.
const DISK_FULL_REASON: &str = "The server is out of disk space.";

/// Deletes exports past their retention together with their files.
#[derive(Clone)]
pub struct ExportExpiry {
    database: Database,
    storage: PhotoStorage,
    retention: Duration,
}

impl ExportExpiry {
    pub const fn new(database: Database, storage: PhotoStorage, retention: Duration) -> Self {
        Self {
            database,
            storage,
            retention,
        }
    }

    /// Removes expired export rows and files. A failure is logged and left for the next run.
    pub async fn run(&self) {
        let exports = self.database.repository::<ExportRepository>();
        let expired = match exports.expire(self.retention).await {
            Ok(expired) => expired,
            Err(error) => {
                warn!(%error, "expired exports could not be deleted");

                return;
            }
        };

        for export in expired {
            let key = ArchiveKey {
                workspace: export.workspace_id,
                gallery: export.gallery_id,
                export: export.id,
            };

            if let Err(error) = self.storage.remove_archive(&key).await {
                warn!(%error, export = %export.id.0, "expired selection ZIP could not be removed");
            }
        }
    }

    /// Removes archives older than `grace` that no export row claims.
    pub async fn sweep_archives(&self, grace: Duration) -> Result<usize, StorageError> {
        let exports = self.database.repository::<ExportRepository>();

        self.storage
            .sweep_archives(grace, async |key: ArchiveKey| {
                // When the database cannot say, the file is kept.
                exports.exists(key.export).await.unwrap_or(true)
            })
            .await
    }
}

/// Serves `zip_export` jobs.
pub struct ExportHandler {
    database: Database,
    storage: PhotoStorage,
    expiry: ExportExpiry,
}

impl ExportHandler {
    pub const fn new(database: Database, storage: PhotoStorage, expiry: ExportExpiry) -> Self {
        Self {
            database,
            storage,
            expiry,
        }
    }

    /// Writes the ZIP, then publishes it as ready only if every snapshot photo still exists.
    ///
    /// A missing original or a full volume fails the export visibly without retries; any other
    /// storage error is retried with the job and fails the export after the last attempt.
    pub async fn run(
        self: Arc<Self>,
        payload: ZipExport,
        job: Job,
    ) -> Result<JobCompletion, HandlerError> {
        let exports = self.database.repository::<ExportRepository>();
        let id = payload.export_id;
        let Some(plan) = exports.plan(id).await.map_err(HandlerError::failed)? else {
            return self.finish(&job).await;
        };
        let key = ArchiveKey {
            workspace: plan.workspace_id,
            gallery: plan.gallery_id,
            export: id,
        };

        match self.storage.build_archive(&key, Self::entries(&plan)).await {
            Ok(()) => {}
            Err(StorageError::NotFound) => return self.fail(&job, id, PHOTO_REMOVED_REASON).await,
            Err(StorageError::DiskFull(_)) => return self.fail(&job, id, DISK_FULL_REASON).await,
            Err(error) => return Err(HandlerError::failed(error)),
        }

        let (completion, update) = exports
            .complete(&job, id)
            .await
            .map_err(HandlerError::failed)?;

        // A claim that is no longer ours leaves the file to the run that now owns the job.
        if completion.is_current() && update != ExportUpdate::Saved {
            self.discard(&key).await;
        }

        info!(export = %id.0, photos = plan.photos.len(), ?update, "export built");
        self.expiry.run().await;

        Ok(completion)
    }

    fn entries(plan: &ExportPlan) -> Vec<ArchiveEntry> {
        let location = PhotoLocation {
            workspace_id: plan.workspace_id,
            gallery_id: plan.gallery_id,
        };

        plan.photos
            .iter()
            .map(|photo| ArchiveEntry {
                source: PhotoKey::with_location(location, photo.id, Variant::Original),
                original_name: photo.original_name.clone(),
            })
            .collect()
    }

    async fn fail(
        &self,
        job: &Job,
        id: ExportId,
        reason: &str,
    ) -> Result<JobCompletion, HandlerError> {
        self.database
            .repository::<ExportRepository>()
            .fail(job, id, reason)
            .await
            .map_err(HandlerError::failed)
    }

    async fn finish(&self, job: &Job) -> Result<JobCompletion, HandlerError> {
        self.database
            .repository::<JobRepository>()
            .complete(job)
            .await
            .map_err(HandlerError::failed)
    }

    async fn discard(&self, key: &ArchiveKey) {
        if let Err(error) = self.storage.remove_archive(key).await {
            warn!(%error, "unwanted selection ZIP could not be removed");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use pp_core::{config::PhotoDir, notifications::PhotoId, types::generate::Generate};
    use pp_database::{
        PgPool,
        export::{ExportRequest, ExportStatus},
        gallery::GalleryRepository,
        jobs::JobKind,
        tests::TestRepository,
    };
    use tempfile::TempDir;

    use super::*;

    const DAY: Duration = Duration::from_secs(86_400);

    struct Fixture {
        _dir: TempDir,
        pool: PgPool,
        database: Database,
        storage: PhotoStorage,
        handler: Arc<ExportHandler>,
        location: PhotoLocation,
    }

    impl Fixture {
        async fn new(pool: PgPool) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let storage = PhotoStorage::open(&PhotoDir::try_from(dir.path().to_owned()).unwrap())
                .await
                .unwrap();
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
            let expiry = ExportExpiry::new(database.clone(), storage.clone(), DAY);

            Self {
                _dir: dir,
                pool,
                handler: Arc::new(ExportHandler::new(
                    database.clone(),
                    storage.clone(),
                    expiry,
                )),
                database,
                storage,
                location: PhotoLocation {
                    workspace_id,
                    gallery_id,
                },
            }
        }

        /// A favorite photo whose original is stored unless `stored` is false.
        async fn favorite(&self, name: &str, stored: bool) -> PhotoId {
            let id = PhotoId::generate();

            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width,
                                     height, status, favorite)
                 VALUES ($1, $2, $3, $4, 5, 2, 2, 'ready', true)",
            )
            .bind(id)
            .bind(self.location.workspace_id)
            .bind(self.location.gallery_id)
            .bind(name)
            .execute(&self.pool)
            .await
            .unwrap();

            if stored {
                let key = PhotoKey::with_location(self.location, id, Variant::Original);
                let mut staged = self.storage.stage().await.unwrap();

                staged.write(b"bytes").await.unwrap();
                staged.commit(&key).await.unwrap();
            }

            id
        }

        async fn request(&self) -> (ExportId, Job) {
            let ExportRequest::Created { export, .. } = self
                .database
                .repository::<ExportRepository>()
                .request(
                    self.location.workspace_id,
                    self.location.gallery_id,
                    NonZeroU64::new(1 << 30).unwrap(),
                )
                .await
                .unwrap()
            else {
                panic!("expected a new export");
            };
            let job = self
                .database
                .repository::<JobRepository>()
                .claim(&[JobKind::ZipExport], Duration::from_secs(60))
                .await
                .unwrap()
                .unwrap();

            (export.id, job)
        }

        async fn run(&self, id: ExportId, job: Job) -> JobCompletion {
            Arc::clone(&self.handler)
                .run(ZipExport { export_id: id }, job)
                .await
                .unwrap()
        }

        async fn status(&self) -> ExportStatus {
            let export = self
                .database
                .repository::<ExportRepository>()
                .latest(self.location.workspace_id, self.location.gallery_id, DAY)
                .await
                .unwrap()
                .unwrap();

            export.status
        }

        fn key(&self, id: ExportId) -> ArchiveKey {
            ArchiveKey {
                workspace: self.location.workspace_id,
                gallery: self.location.gallery_id,
                export: id,
            }
        }

        async fn has_zip(&self, id: ExportId) -> bool {
            self.storage.read_archive(&self.key(id)).await.is_ok()
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_build_publishes_the_zip_and_marks_the_export_ready(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.favorite("a.jpg", true).await;
        fixture.favorite("b.jpg", true).await;

        let (id, job) = fixture.request().await;

        assert!(fixture.run(id, job).await.is_current());
        assert_eq!(fixture.status().await, ExportStatus::Ready);
        assert!(fixture.has_zip(id).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_missing_original_fails_the_export_without_a_retry(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.favorite("a.jpg", true).await;
        fixture.favorite("b.jpg", false).await;

        let (id, job) = fixture.request().await;

        assert!(fixture.run(id, job).await.is_current());
        assert_eq!(
            fixture.status().await,
            ExportStatus::Failed(PHOTO_REMOVED_REASON.to_owned())
        );
        assert!(!fixture.has_zip(id).await);
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

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_photo_deleted_during_the_build_fails_the_export_and_drops_the_file(pool: PgPool) {
        let fixture = Fixture::new(pool.clone()).await;
        let photo = fixture.favorite("a.jpg", true).await;
        let (id, job) = fixture.request().await;

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(photo)
            .execute(&pool)
            .await
            .unwrap();

        assert!(fixture.run(id, job).await.is_current());
        assert_eq!(
            fixture.status().await,
            ExportStatus::Failed(PHOTO_REMOVED_REASON.to_owned())
        );
        assert!(!fixture.has_zip(id).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_job_whose_export_is_gone_just_finishes(pool: PgPool) {
        let fixture = Fixture::new(pool.clone()).await;

        fixture.favorite("a.jpg", true).await;

        let (id, job) = fixture.request().await;

        sqlx::query("DELETE FROM exports")
            .execute(&pool)
            .await
            .unwrap();

        assert!(fixture.run(id, job).await.is_current());
        assert!(!fixture.has_zip(id).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_rerun_after_an_interrupted_build_replaces_the_file(pool: PgPool) {
        let fixture = Fixture::new(pool.clone()).await;

        fixture.favorite("a.jpg", true).await;

        let (id, job) = fixture.request().await;

        fixture
            .storage
            .build_archive(&fixture.key(id), Vec::new())
            .await
            .unwrap();

        assert!(fixture.run(id, job).await.is_current());
        assert_eq!(fixture.status().await, ExportStatus::Ready);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn expiry_removes_old_exports_and_their_files(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.favorite("a.jpg", true).await;

        let (id, job) = fixture.request().await;

        let _ = fixture.run(id, job).await;

        let expiry = ExportExpiry::new(
            fixture.database.clone(),
            fixture.storage.clone(),
            Duration::ZERO,
        );

        expiry.run().await;

        assert!(!fixture.has_zip(id).await);
        assert!(
            !fixture
                .database
                .repository::<ExportRepository>()
                .exists(id)
                .await
                .unwrap()
        );
    }
}
