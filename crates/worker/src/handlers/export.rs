//! Builds the ZIP of a photographer's selection from its stored snapshot.
//!
//! [`ExportHandler`] writes the archive; [`ExportFinalizer`] records the outcome together with
//! the deletion of the job.

use std::sync::Arc;

use pp_database::{
    Database,
    export::{ExportFailure, ExportId, ExportPlan, ExportRepository, ExportUpdate},
    jobs::{Job, JobCompletion, JobRepository, ZipExport},
    photo::PhotoLocation,
};
use pp_storage::{ArchiveEntry, ArchiveKey, PhotoKey, PhotoStorage, StorageError, Variant};
use tracing::{info, warn};

use crate::handlers::{Finalizer, HandlerError};

/// What building an export produced, for [`ExportFinalizer`] to record.
#[derive(Debug)]
pub enum ExportOutput {
    /// The export no longer exists, so there is nothing to record.
    Gone,
    /// The archive of `photos` photos was written.
    Built {
        id: ExportId,
        key: ArchiveKey,
        photos: usize,
    },
    /// The archive cannot be built, and retrying cannot change that.
    Failed {
        id: ExportId,
        failure: ExportFailure,
    },
}

/// Serves `zip_export` jobs: writes the archive of a selection.
pub struct ExportHandler {
    database: Database,
    storage: PhotoStorage,
}

impl ExportHandler {
    pub const fn new(database: Database, storage: PhotoStorage) -> Self {
        Self { database, storage }
    }

    /// Writes the ZIP. The finalizer publishes it as ready only if every snapshot photo still
    /// exists.
    ///
    /// A missing original or a full volume fails the export visibly without retries; any other
    /// storage error is retried with the job and fails the export after the last attempt.
    pub async fn run(self: Arc<Self>, payload: ZipExport) -> Result<ExportOutput, HandlerError> {
        let id = payload.export_id;
        let plan = self
            .database
            .repository::<ExportRepository>()
            .plan(id)
            .await
            .map_err(HandlerError::failed)?;
        let Some(plan) = plan else {
            return Ok(ExportOutput::Gone);
        };
        let key = ArchiveKey {
            workspace: plan.workspace_id,
            gallery: plan.gallery_id,
            export: id,
        };

        match self.storage.build_archive(&key, Self::entries(&plan)).await {
            Ok(()) => Ok(ExportOutput::Built {
                id,
                key,
                photos: plan.photos.len(),
            }),
            Err(StorageError::NotFound) => Ok(ExportOutput::Failed {
                id,
                failure: ExportFailure::PhotoRemoved,
            }),
            Err(StorageError::DiskFull(_)) => Ok(ExportOutput::Failed {
                id,
                failure: ExportFailure::DiskFull,
            }),
            Err(error) => Err(HandlerError::failed(error)),
        }
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
}

/// Records the outcome of `zip_export` jobs together with the deletion of the job.
pub struct ExportFinalizer {
    database: Database,
    storage: PhotoStorage,
}

impl ExportFinalizer {
    pub const fn new(database: Database, storage: PhotoStorage) -> Self {
        Self { database, storage }
    }

    async fn discard(&self, key: &ArchiveKey) {
        if let Err(error) = self.storage.remove_archive(key).await {
            warn!(%error, "unwanted selection ZIP could not be removed");
        }
    }
}

impl Finalizer for ExportFinalizer {
    type Output = ExportOutput;

    async fn finalize(
        &self,
        job: &Job,
        output: ExportOutput,
    ) -> Result<JobCompletion, HandlerError> {
        let exports = self.database.repository::<ExportRepository>();

        match output {
            ExportOutput::Gone => self
                .database
                .repository::<JobRepository>()
                .complete(job)
                .await
                .map_err(HandlerError::failed),
            ExportOutput::Failed { id, failure } => exports
                .fail(job, id, failure)
                .await
                .map_err(HandlerError::failed),
            ExportOutput::Built { id, key, photos } => {
                let (completion, update) = exports
                    .complete(job, id)
                    .await
                    .map_err(HandlerError::failed)?;

                // A claim that is no longer ours leaves the file to the run that now owns the job.
                if completion.is_current() && update != ExportUpdate::Saved {
                    self.discard(&key).await;
                }

                info!(export = %id.0, photos, ?update, "export built");

                Ok(completion)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, time::Duration};

    use pp_core::{config::PhotoDir, types::generate::Generate};
    use pp_database::photo::PhotoId;
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
        finalizer: ExportFinalizer,
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
            Self {
                _dir: dir,
                pool,
                handler: Arc::new(ExportHandler::new(database.clone(), storage.clone())),
                finalizer: ExportFinalizer::new(database.clone(), storage.clone()),
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
                                     height, status, favorited_at)
                 VALUES ($1, $2, $3, $4, 5, 2, 2, 'ready', now())",
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
                    Duration::from_secs(86_400),
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
            let output = Arc::clone(&self.handler)
                .run(ZipExport { export_id: id })
                .await
                .unwrap();

            self.finalizer.finalize(&job, output).await.unwrap()
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
            ExportStatus::Failed(ExportFailure::PhotoRemoved)
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
            ExportStatus::Failed(ExportFailure::PhotoRemoved)
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
}
