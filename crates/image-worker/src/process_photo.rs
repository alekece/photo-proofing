//! Turns an accepted original into the thumbnail and preview that clients see.

use std::{num::NonZeroU64, sync::Arc};

use pp_core::notifications::PhotoId;
use pp_database::{
    Database,
    jobs::ProcessPhoto,
    photo::{PhotoLocation, PhotoRepository},
};
use pp_storage::{PhotoKey, PhotoStorage, Variant};
use tokio::{io::AsyncReadExt, task};
use tracing::warn;

use crate::{
    handlers::HandlerError,
    render::{self, RenderError, Rendered},
};

/// Shown to the photographer when the original cannot be decoded.
pub const UNREADABLE_REASON: &str = "The image could not be read.";
/// Shown to the photographer when the original exceeds the worker's pixel limit.
pub const TOO_LARGE_REASON: &str = "The image is too large to process.";

/// What the handler needs, shared by every job.
pub struct ProcessContext {
    pub database: Database,
    pub storage: PhotoStorage,
    pub max_pixels: NonZeroU64,
}

/// Renders and publishes the derivatives of one photo, then marks it ready.
///
/// An original that cannot be rendered fails the photo for good and completes the job, since
/// retrying cannot change the bytes. Database, storage and library faults are returned for
/// retry.
pub async fn process_photo(
    context: Arc<ProcessContext>,
    payload: ProcessPhoto,
) -> Result<(), HandlerError> {
    let id = payload.photo_id;
    let photos = context.database.repository::<PhotoRepository>();

    // A deleted photo, or one already finished by an earlier run, needs nothing more.
    let Some(location) = photos.locate(id).await.map_err(failed)? else {
        return Ok(());
    };

    if !photos.mark_processing(id).await.map_err(failed)? {
        return Ok(());
    }

    let original = read_original(&context.storage, location, id).await?;
    let max_pixels = context.max_pixels;
    let rendered = task::spawn_blocking(move || render::render(&original, max_pixels))
        .await
        .map_err(failed)?;

    let reason = match rendered {
        Ok(rendered) => return publish(&context, location, id, rendered).await,
        Err(RenderError::Unreadable) => UNREADABLE_REASON,
        Err(RenderError::TooLarge) => TOO_LARGE_REASON,
        Err(error @ RenderError::Library(_)) => return Err(failed(error)),
    };

    warn!(photo = %id, reason, "photo cannot be processed");
    photos.mark_failed(id, reason).await.map_err(failed)?;

    Ok(())
}

/// Stores both derivatives and marks the photo ready. If the photo was deleted meanwhile, the
/// derivatives just written are removed so late work leaves nothing behind; a photo that still
/// exists keeps them.
async fn publish(
    context: &ProcessContext,
    location: PhotoLocation,
    id: PhotoId,
    rendered: Rendered,
) -> Result<(), HandlerError> {
    let derivatives = [
        (Variant::Thumbnail, rendered.thumbnail),
        (Variant::Preview, rendered.preview),
    ];

    for (variant, bytes) in &derivatives {
        let mut staged = context.storage.stage().await.map_err(failed)?;

        staged.write(bytes).await.map_err(failed)?;
        staged
            .commit(&key(location, id, *variant))
            .await
            .map_err(failed)?;
    }

    let photos = context.database.repository::<PhotoRepository>();

    // A photo another run already finished keeps the files it serves.
    if !photos.mark_ready(id).await.map_err(failed)?
        && photos.locate(id).await.map_err(failed)?.is_none()
    {
        for (variant, _) in &derivatives {
            // Best effort: a missing file is already the desired outcome.
            let _ = context.storage.remove(&key(location, id, *variant)).await;
        }
    }

    Ok(())
}

async fn read_original(
    storage: &PhotoStorage,
    location: PhotoLocation,
    id: PhotoId,
) -> Result<Vec<u8>, HandlerError> {
    let mut file = storage
        .read(&key(location, id, Variant::Original))
        .await
        .map_err(failed)?;
    let mut original = Vec::new();

    file.read_to_end(&mut original).await.map_err(failed)?;

    Ok(original)
}

fn key(location: PhotoLocation, photo: PhotoId, variant: Variant) -> PhotoKey {
    PhotoKey {
        workspace: location.workspace_id,
        gallery: location.gallery_id,
        photo,
        variant,
    }
}

fn failed(error: impl std::error::Error + Send + Sync + 'static) -> HandlerError {
    HandlerError::Failed(Box::new(error))
}

#[cfg(test)]
mod tests {
    use std::{
        fs, future,
        num::{NonZeroU32, NonZeroUsize},
        path::Path,
        time::Duration,
    };

    use libvips::ops::{gaussnoise, jpegsave_buffer};
    use pp_core::{config::PhotoDir, types::generate::Generate};
    use pp_database::{
        PgPool,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        jobs::{Backoff, JobKind},
        photo::{NewPhoto, PhotoDimensions, PhotoStatus},
        photo_intake::PhotoIntake,
    };
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;
    use uuid::Uuid;

    use super::*;
    use crate::{
        handlers::HandlerRegistry,
        runtime::{RuntimeOptions, Worker},
    };

    struct Fixture {
        dir: TempDir,
        context: Arc<ProcessContext>,
        location: PhotoLocation,
        id: PhotoId,
        original: Vec<u8>,
    }

    impl Fixture {
        /// Accepts a photo with `original` stored as its upload.
        async fn new(pool: &PgPool, original: Vec<u8>) -> Self {
            render::install().unwrap();

            let dir = tempfile::tempdir().unwrap();
            let storage = PhotoStorage::open(&PhotoDir::try_from(dir.path().to_owned()).unwrap())
                .await
                .unwrap();
            let database = Database::new(pool.clone());
            let workspace_id = WorkspaceId(Uuid::new_v4());

            sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
                .bind(workspace_id)
                .execute(pool)
                .await
                .unwrap();

            let gallery_id = database
                .repository::<GalleryRepository>()
                .create(workspace_id)
                .await
                .unwrap()
                .id;
            let id = PhotoId::generate();

            database
                .repository::<PhotoIntake>()
                .try_accept(NewPhoto {
                    id,
                    workspace_id,
                    gallery_id,
                    original_name: "proof.jpg".to_owned(),
                    size: NonZeroU64::new(original.len() as u64).unwrap(),
                    dimensions: PhotoDimensions {
                        width: NonZeroU32::new(600).unwrap(),
                        height: NonZeroU32::new(400).unwrap(),
                    },
                })
                .await
                .unwrap()
                .unwrap();

            let location = PhotoLocation {
                workspace_id,
                gallery_id,
            };
            let mut staged = storage.stage().await.unwrap();

            staged.write(&original).await.unwrap();
            staged
                .commit(&key(location, id, Variant::Original))
                .await
                .unwrap();

            Self {
                dir,
                context: Arc::new(ProcessContext {
                    database,
                    storage,
                    max_pixels: NonZeroU64::new(100_000_000).unwrap(),
                }),
                location,
                id,
                original,
            }
        }

        async fn run(&self) -> Result<(), HandlerError> {
            process_photo(self.context.clone(), ProcessPhoto { photo_id: self.id }).await
        }

        async fn status(&self) -> Option<PhotoStatus> {
            self.context
                .database
                .repository::<PhotoRepository>()
                .get(
                    self.location.workspace_id,
                    self.location.gallery_id,
                    self.id,
                )
                .await
                .unwrap()
                .map(|photo| photo.status)
        }

        fn gallery_dir(&self) -> std::path::PathBuf {
            gallery_dir(
                self.dir.path(),
                self.location.workspace_id,
                self.location.gallery_id,
            )
        }

        fn file(&self, variant: Variant) -> Option<Vec<u8>> {
            fs::read(self.gallery_dir().join(format!("{}-{variant}", self.id))).ok()
        }

        fn files(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.gallery_dir())
                .map(|entries| {
                    entries
                        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                        .collect()
                })
                .unwrap_or_default();

            names.sort();
            names
        }
    }

    fn gallery_dir(root: &Path, workspace: WorkspaceId, gallery: GalleryId) -> std::path::PathBuf {
        root.join(workspace.0.to_string())
            .join(gallery.0.to_string())
    }

    fn jpeg() -> Vec<u8> {
        jpegsave_buffer(&gaussnoise(600, 400).unwrap()).unwrap()
    }

    fn dimensions(bytes: &[u8]) -> (i32, i32) {
        let image = libvips::VipsImage::new_from_buffer(bytes, "").unwrap();

        (image.get_width(), image.get_height())
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn processing_a_photo_publishes_both_derivatives_and_marks_it_ready(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;

        fixture.run().await.unwrap();

        assert_eq!(fixture.status().await, Some(PhotoStatus::Ready));
        assert_eq!(fixture.file(Variant::Original).unwrap(), fixture.original);
        assert_eq!(
            dimensions(&fixture.file(Variant::Thumbnail).unwrap()),
            (400, 267)
        );
        assert_eq!(
            dimensions(&fixture.file(Variant::Preview).unwrap()),
            (600, 400)
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn corrupt_original_fails_permanently_with_a_safe_reason(pool: PgPool) {
        let fixture = Fixture::new(&pool, b"not a jpeg at all".to_vec()).await;

        fixture.run().await.unwrap();

        assert_eq!(
            fixture.status().await,
            Some(PhotoStatus::Failed(UNREADABLE_REASON.to_owned()))
        );
        assert_eq!(fixture.file(Variant::Thumbnail), None);
        assert_eq!(fixture.file(Variant::Preview), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn oversized_original_fails_permanently_with_a_safe_reason(pool: PgPool) {
        let mut fixture = Fixture::new(&pool, jpeg()).await;

        Arc::get_mut(&mut fixture.context).unwrap().max_pixels =
            NonZeroU64::new(600 * 400 - 1).unwrap();
        fixture.run().await.unwrap();

        assert_eq!(
            fixture.status().await,
            Some(PhotoStatus::Failed(TOO_LARGE_REASON.to_owned()))
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn storage_error_is_returned_for_retry(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;

        fs::remove_file(fixture.gallery_dir().join(format!(
            "{}-{}",
            fixture.id,
            Variant::Original
        )))
        .unwrap();

        assert!(matches!(fixture.run().await, Err(HandlerError::Failed(_))));
        assert_eq!(fixture.status().await, Some(PhotoStatus::Processing));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn rerun_replaces_derivatives_without_duplicates(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;

        fixture.run().await.unwrap();
        sqlx::query("UPDATE photos SET status = 'waiting' WHERE id = $1")
            .bind(fixture.id)
            .execute(&pool)
            .await
            .unwrap();
        fixture.run().await.unwrap();

        assert_eq!(fixture.status().await, Some(PhotoStatus::Ready));
        assert_eq!(
            fixture.files(),
            [Variant::Original, Variant::Preview, Variant::Thumbnail]
                .map(|variant| format!("{}-{variant}", fixture.id))
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn finished_photo_is_not_reprocessed(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let photos = fixture.context.database.repository::<PhotoRepository>();

        photos
            .mark_failed(fixture.id, "earlier failure")
            .await
            .unwrap();
        fixture.run().await.unwrap();

        assert_eq!(
            fixture.status().await,
            Some(PhotoStatus::Failed("earlier failure".to_owned()))
        );
        assert_eq!(fixture.file(Variant::Thumbnail), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn deleted_photo_is_skipped(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;

        sqlx::query("DELETE FROM galleries WHERE id = $1")
            .bind(fixture.location.gallery_id)
            .execute(&pool)
            .await
            .unwrap();
        fixture.run().await.unwrap();

        assert_eq!(fixture.file(Variant::Thumbnail), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn late_work_for_a_deleted_photo_leaves_nothing(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let rendered = render::render(&fixture.original, fixture.context.max_pixels).unwrap();

        sqlx::query("DELETE FROM galleries WHERE id = $1")
            .bind(fixture.location.gallery_id)
            .execute(&pool)
            .await
            .unwrap();
        publish(&fixture.context, fixture.location, fixture.id, rendered)
            .await
            .unwrap();

        assert_eq!(fixture.file(Variant::Thumbnail), None);
        assert_eq!(fixture.file(Variant::Preview), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn late_work_for_a_ready_photo_keeps_its_derivatives(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let rendered = render::render(&fixture.original, fixture.context.max_pixels).unwrap();

        fixture.run().await.unwrap();
        publish(&fixture.context, fixture.location, fixture.id, rendered)
            .await
            .unwrap();

        assert_eq!(fixture.status().await, Some(PhotoStatus::Ready));
        assert!(fixture.file(Variant::Thumbnail).is_some());
        assert!(fixture.file(Variant::Preview).is_some());
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn worker_restart_mid_job_recovers(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let options = RuntimeOptions {
            poll_interval: Duration::from_millis(30),
            lease: Duration::from_millis(600),
            concurrency: NonZeroUsize::new(1).unwrap(),
            shutdown_grace: Duration::from_millis(50),
            backoff: Backoff::new(Duration::from_millis(50), Duration::from_millis(100)),
        };

        // The first worker starts the photo, then never finishes and is stopped.
        let stuck = {
            let context = fixture.context.clone();

            HandlerRegistry::default().register(
                JobKind::ProcessPhoto,
                move |payload: ProcessPhoto| {
                    let context = context.clone();

                    async move {
                        context
                            .database
                            .repository::<PhotoRepository>()
                            .mark_processing(payload.photo_id)
                            .await
                            .unwrap();
                        future::pending().await
                    }
                },
            )
        };
        let shutdown = CancellationToken::new();
        let first = tokio::spawn(
            Worker::new(fixture.context.database.clone(), stuck, options).run(shutdown.clone()),
        );

        while fixture.status().await != Some(PhotoStatus::Processing) {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        shutdown.cancel();
        first.await.unwrap();

        let context = fixture.context.clone();
        let handlers = HandlerRegistry::default().register(JobKind::ProcessPhoto, move |payload| {
            process_photo(context.clone(), payload)
        });
        let shutdown = CancellationToken::new();
        let second = tokio::spawn(
            Worker::new(fixture.context.database.clone(), handlers, options).run(shutdown.clone()),
        );

        tokio::time::timeout(Duration::from_secs(10), async {
            while fixture.status().await != Some(PhotoStatus::Ready) {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();

        shutdown.cancel();
        second.await.unwrap();
    }
}
