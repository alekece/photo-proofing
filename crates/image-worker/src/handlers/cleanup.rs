//! Removes the files of deleted galleries and photos, and of anything else nothing owns.

use std::{sync::Arc, time::Duration};

use pp_database::{
    Database,
    jobs::{CleanupGallery, CleanupPhotos, Job, JobCompletion, JobKind, JobRepository},
    photo::{PhotoLocation, PhotoRepository},
};
use pp_storage::PhotoStorage;
use thiserror::Error;
use tracing::{info, warn};

use crate::handlers::{HandlerError, selection_export::ExportExpiry};

#[derive(Debug, Error)]
#[error("{count} photo files could not be removed")]
struct RemovalFailed {
    count: usize,
}

/// Serves the cleanup jobs that follow a deletion. Every job is idempotent: files already gone
/// count as removed.
pub struct CleanupHandler {
    database: Database,
    storage: PhotoStorage,
    /// A file younger than this is never swept, so an upload still being finished is safe.
    orphan_grace: Duration,
    /// A sweep that follows another one by less than this is skipped.
    sweep_interval: Duration,
    expiry: ExportExpiry,
}

impl CleanupHandler {
    pub const fn new(
        database: Database,
        storage: PhotoStorage,
        orphan_grace: Duration,
        sweep_interval: Duration,
        expiry: ExportExpiry,
    ) -> Self {
        Self {
            database,
            storage,
            orphan_grace,
            sweep_interval,
            expiry,
        }
    }

    /// Removes a deleted gallery's whole directory.
    pub async fn remove_gallery(
        self: Arc<Self>,
        payload: CleanupGallery,
        job: Job,
    ) -> Result<JobCompletion, HandlerError> {
        self.storage
            .remove_gallery(payload.workspace_id, payload.gallery_id)
            .await
            .map_err(HandlerError::failed)?;

        self.finish(&job).await
    }

    /// Removes the files of deleted photos. Failures are retried with the rest of the job.
    pub async fn remove_photos(
        self: Arc<Self>,
        payload: CleanupPhotos,
        job: Job,
    ) -> Result<JobCompletion, HandlerError> {
        let location = PhotoLocation {
            workspace_id: payload.workspace_id,
            gallery_id: payload.gallery_id,
        };
        let failures = self.storage.remove_many(location, &payload.photo_ids).await;

        for (key, error) in &failures {
            warn!(photo = %key.photo, %error, "photo file could not be removed");
        }

        if !failures.is_empty() {
            return Err(HandlerError::failed(RemovalFailed {
                count: failures.len(),
            }));
        }

        self.finish(&job).await
    }

    /// Removes old files that no photo owns, whatever left them behind. A sweep that comes too
    /// soon after the last successful one finishes without doing anything.
    ///
    /// When the database cannot say, the sweep runs: it is harmless, only wasteful.
    pub async fn sweep_orphans(
        self: Arc<Self>,
        _payload: (),
        job: Job,
    ) -> Result<JobCompletion, HandlerError> {
        let jobs = self.database.repository::<JobRepository>();

        if jobs
            .ran_within(JobKind::SweepOrphans, self.sweep_interval)
            .await
            .unwrap_or(false)
        {
            info!("orphaned photo file sweep skipped, the last one is recent");

            return self.finish(&job).await;
        }

        let photos = self.database.repository::<PhotoRepository>();
        // When the database cannot say, the file is kept.
        let is_stored = async |_: PhotoLocation, photo| {
            photos
                .locate(photo)
                .await
                .map_or(true, |location| location.is_some())
        };
        let removed = self
            .storage
            .sweep_orphans(self.orphan_grace, is_stored)
            .await
            .map_err(HandlerError::failed)?;

        info!(removed, "orphaned photo files swept");

        self.expiry.run().await;

        let archives = self
            .expiry
            .sweep_archives(self.orphan_grace)
            .await
            .map_err(HandlerError::failed)?;

        info!(archives, "orphaned selection ZIPs swept");

        jobs.complete_recorded(&job)
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
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU16;

    use pp_core::{config::PhotoDir, notifications::PhotoId, types::generate::Generate};
    use pp_database::{
        PgPool, gallery::GalleryRepository, jobs::JobPayload, tests::TestRepository,
    };
    use pp_storage::{PhotoKey, StorageError, Variant};
    use tempfile::TempDir;

    use super::*;

    struct Fixture {
        _dir: TempDir,
        handler: Arc<CleanupHandler>,
        location: PhotoLocation,
    }

    impl Fixture {
        async fn new(pool: &PgPool) -> Self {
            let day = Duration::from_secs(24 * 60 * 60);

            Self::with_sweep(pool, day, day).await
        }

        async fn with_sweep(
            pool: &PgPool,
            orphan_grace: Duration,
            sweep_interval: Duration,
        ) -> Self {
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
                handler: Arc::new(CleanupHandler::new(
                    database.clone(),
                    storage.clone(),
                    orphan_grace,
                    sweep_interval,
                    ExportExpiry::new(database, storage, Duration::from_secs(3600)),
                )),
                location: PhotoLocation {
                    workspace_id,
                    gallery_id,
                },
            }
        }

        async fn store(&self, photo: PhotoId, variant: Variant) -> PhotoKey {
            let key = PhotoKey::with_location(self.location, photo, variant);
            let mut staged = self.handler.storage.stage().await.unwrap();

            staged.write(b"x").await.unwrap();
            staged.commit(&key).await.unwrap();

            key
        }

        async fn claim(&self, payload: JobPayload, kind: JobKind) -> Job {
            let jobs = self.handler.database.repository::<JobRepository>();

            jobs.enqueue(payload, NonZeroU16::new(3).unwrap())
                .await
                .unwrap();

            jobs.claim(&[kind], Duration::from_secs(60))
                .await
                .unwrap()
                .unwrap()
        }

        async fn is_stored(&self, key: &PhotoKey) -> bool {
            !matches!(
                self.handler.storage.read(key).await,
                Err(StorageError::NotFound)
            )
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn gallery_cleanup_removes_the_directory_and_finishes_its_job(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let key = fixture.store(PhotoId::generate(), Variant::Thumbnail).await;
        let payload = CleanupGallery {
            workspace_id: fixture.location.workspace_id,
            gallery_id: fixture.location.gallery_id,
        };
        let job = fixture
            .claim(JobPayload::CleanupGallery(payload), JobKind::CleanupGallery)
            .await;
        let completion = Arc::clone(&fixture.handler)
            .remove_gallery(payload, job)
            .await
            .unwrap();

        assert!(completion.is_current());
        assert!(!fixture.is_stored(&key).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn photo_cleanup_removes_only_the_named_photos(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let doomed = PhotoId::generate();
        let gone = fixture.store(doomed, Variant::Original).await;
        let kept = fixture.store(PhotoId::generate(), Variant::Original).await;
        let payload = CleanupPhotos {
            workspace_id: fixture.location.workspace_id,
            gallery_id: fixture.location.gallery_id,
            photo_ids: vec![doomed, PhotoId::generate()],
        };
        let job = fixture
            .claim(
                JobPayload::CleanupPhotos(payload.clone()),
                JobKind::CleanupPhotos,
            )
            .await;

        let completion = Arc::clone(&fixture.handler)
            .remove_photos(payload, job)
            .await
            .unwrap();

        assert!(completion.is_current());

        assert!(!fixture.is_stored(&gone).await);
        assert!(fixture.is_stored(&kept).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_sweep_keeps_recent_files_and_finishes_its_job(pool: PgPool) {
        let fixture = Fixture::new(&pool).await;
        let key = fixture.store(PhotoId::generate(), Variant::Original).await;
        let job = fixture
            .claim(JobPayload::SweepOrphans, JobKind::SweepOrphans)
            .await;
        let completion = Arc::clone(&fixture.handler)
            .sweep_orphans((), job)
            .await
            .unwrap();

        assert!(completion.is_current());
        assert!(fixture.is_stored(&key).await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_sweep_removes_old_orphans_and_records_its_run(pool: PgPool) {
        let fixture = Fixture::with_sweep(&pool, Duration::ZERO, Duration::from_secs(3600)).await;
        let key = fixture.store(PhotoId::generate(), Variant::Original).await;
        let job = fixture
            .claim(JobPayload::SweepOrphans, JobKind::SweepOrphans)
            .await;

        let completion = Arc::clone(&fixture.handler)
            .sweep_orphans((), job)
            .await
            .unwrap();

        assert!(completion.is_current());
        assert!(!fixture.is_stored(&key).await);
        assert!(
            fixture
                .handler
                .database
                .repository::<JobRepository>()
                .ran_within(JobKind::SweepOrphans, Duration::from_secs(3600))
                .await
                .unwrap()
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_sweep_right_after_another_is_skipped_but_still_finishes(pool: PgPool) {
        let fixture = Fixture::with_sweep(&pool, Duration::ZERO, Duration::from_secs(3600)).await;
        let first = fixture
            .claim(JobPayload::SweepOrphans, JobKind::SweepOrphans)
            .await;

        assert!(
            Arc::clone(&fixture.handler)
                .sweep_orphans((), first)
                .await
                .unwrap()
                .is_current()
        );

        let key = fixture.store(PhotoId::generate(), Variant::Original).await;
        let second = fixture
            .claim(JobPayload::SweepOrphans, JobKind::SweepOrphans)
            .await;
        let completion = Arc::clone(&fixture.handler)
            .sweep_orphans((), second)
            .await
            .unwrap();

        assert!(completion.is_current());
        assert!(fixture.is_stored(&key).await);
    }
}
