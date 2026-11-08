//! Removes the files of deleted galleries and photos.
//!
//! Handlers only remove files; the runtime finishes the job with `CompleteJob`.

use std::sync::Arc;

use pp_database::{
    jobs::{CleanupGallery, CleanupPhotos},
    photo::PhotoLocation,
};
use pp_storage::PhotoStorage;
use thiserror::Error;
use tracing::warn;

use crate::handlers::HandlerError;

#[derive(Debug, Error)]
#[error("{count} photo files could not be removed")]
struct RemovalFailed {
    count: usize,
}

/// Serves the cleanup jobs that follow a deletion. Every job is idempotent: files already gone
/// count as removed.
pub struct CleanupHandler {
    storage: PhotoStorage,
}

impl CleanupHandler {
    pub const fn new(storage: PhotoStorage) -> Self {
        Self { storage }
    }

    /// Removes a deleted gallery's whole directory.
    pub async fn remove_gallery(
        self: Arc<Self>,
        payload: CleanupGallery,
    ) -> Result<(), HandlerError> {
        self.storage
            .remove_gallery(payload.workspace_id, payload.gallery_id)
            .await
            .map_err(HandlerError::failed)
    }

    /// Removes the files of deleted photos. Failures are retried with the rest of the job.
    pub async fn remove_photos(
        self: Arc<Self>,
        payload: CleanupPhotos,
    ) -> Result<(), HandlerError> {
        let location = PhotoLocation {
            workspace_id: payload.workspace_id,
            gallery_id: payload.gallery_id,
        };
        let failures = self.storage.remove_many(location, &payload.photo_ids).await;

        for (key, error) in &failures {
            warn!(photo = %key.photo, %error, "photo file could not be removed");
        }

        if failures.is_empty() {
            return Ok(());
        }

        Err(HandlerError::failed(RemovalFailed {
            count: failures.len(),
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU16, time::Duration};

    use pp_core::{config::PhotoDir, notifications::PhotoId, types::generate::Generate};
    use pp_database::{
        Database, PgPool,
        gallery::GalleryRepository,
        jobs::{Job, JobKind, JobPayload, JobRepository},
        tests::TestRepository,
    };
    use pp_storage::{PhotoKey, StorageError, Variant};
    use tempfile::TempDir;

    use super::*;
    use crate::handlers::{CompleteJob, Finalizer};

    struct Fixture {
        _dir: TempDir,
        handler: Arc<CleanupHandler>,
        database: Database,
        location: PhotoLocation,
    }

    impl Fixture {
        async fn new(pool: &PgPool) -> Self {
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
                handler: Arc::new(CleanupHandler::new(storage)),
                database,
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
            let jobs = self.database.repository::<JobRepository>();

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

        Arc::clone(&fixture.handler)
            .remove_gallery(payload)
            .await
            .unwrap();

        let completion = CompleteJob::new(fixture.database.clone())
            .finalize(&job, ())
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

        Arc::clone(&fixture.handler)
            .remove_photos(payload)
            .await
            .unwrap();

        let completion = CompleteJob::new(fixture.database.clone())
            .finalize(&job, ())
            .await
            .unwrap();

        assert!(completion.is_current());

        assert!(!fixture.is_stored(&gone).await);
        assert!(fixture.is_stored(&kept).await);
    }
}
