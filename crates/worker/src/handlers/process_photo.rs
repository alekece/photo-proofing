//! Turns an accepted original into the thumbnail and preview that clients see.
//!
//! [`ProcessPhotoHandler`] renders and stages the files; [`ProcessPhotoFinalizer`] publishes them
//! with the photo's result and the job's deletion in one transaction, so the handler never
//! touches the job.

use std::sync::Arc;

use pp_database::photo::PhotoId;
use pp_database::{
    Database,
    jobs::{
        Job, JobCompletion, JobRepository, PhotoResult, PhotoUpdate, ProcessPhoto, Publication,
    },
    photo::{PhotoLocation, PhotoRepository},
    settings::{SettingsRepository, WatermarkStamp},
};
use pp_storage::{PhotoKey, PhotoStorage, StagedPhoto, Variant};
use tokio::{io::AsyncReadExt, task};
use tracing::warn;

use crate::{
    handlers::{Finalizer, HandlerError},
    render::{ImageRenderer, ProofRender, RenderError, RenderedProof},
};

/// Shown to the photographer when the original cannot be decoded.
pub const UNREADABLE_REASON: &str = "The image could not be read.";
/// Shown to the photographer when the original exceeds the worker's pixel limit.
pub const TOO_LARGE_REASON: &str = "The image is too large to process.";

/// What processing a photo produced, for [`ProcessPhotoFinalizer`] to record.
#[derive(Debug)]
pub enum PhotoOutput {
    /// Nothing to record: the photo is deleted or was already finished.
    Skipped,
    /// The original cannot be rendered, and retrying cannot change that.
    Failed { id: PhotoId, reason: &'static str },
    /// Derivatives staged for publication.
    Rendered(StagedProof),
}

/// The staged derivatives of one render and what they were rendered for.
#[derive(Debug)]
pub struct StagedProof {
    id: PhotoId,
    stamp: WatermarkStamp,
    files: Vec<(PhotoKey, StagedPhoto)>,
}

/// Serves `process_photo` jobs: renders a photo's derivatives and stages them.
pub struct ProcessPhotoHandler {
    database: Database,
    storage: PhotoStorage,
    renderer: ImageRenderer,
}

impl ProcessPhotoHandler {
    pub const fn new(database: Database, storage: PhotoStorage, renderer: ImageRenderer) -> Self {
        Self {
            database,
            storage,
            renderer,
        }
    }

    /// Renders the derivatives of one photo and stages them without making them visible.
    ///
    /// The proofs carry the watermark that is current when rendering starts. The finalizer
    /// publishes them only if it is still current then; otherwise the change that outdated them
    /// already queued a job for the photo.
    ///
    /// An original that cannot be rendered is reported as [`PhotoOutput::Failed`], since
    /// retrying cannot change the bytes. Database, storage and library faults are returned for
    /// retry.
    pub async fn run(self: Arc<Self>, payload: ProcessPhoto) -> Result<PhotoOutput, HandlerError> {
        let id = payload.photo_id;
        let photos = self.database.repository::<PhotoRepository>();

        // A deleted photo, or one already finished by an earlier run, needs nothing more.
        let Some(location) = photos.locate(id).await.map_err(HandlerError::failed)? else {
            return Ok(PhotoOutput::Skipped);
        };

        if !photos
            .mark_processing(id)
            .await
            .map_err(HandlerError::failed)?
        {
            return Ok(PhotoOutput::Skipped);
        }

        // Read after `mark_processing`: a change from here on resets the photo and queues
        // another job, and publication checks the stamp, so no outdated proof is ever shown.
        let Some(proof) = self
            .database
            .repository::<SettingsRepository>()
            .proof_watermark(id)
            .await
            .map_err(HandlerError::failed)?
        else {
            return Ok(PhotoOutput::Skipped);
        };
        let stamp = proof.stamp;
        let original = self.read_original(location, id).await?;
        let renderer = self.renderer;
        let rendered = task::spawn_blocking(move || {
            if stamp.revision().is_none() {
                renderer
                    .render(&original, proof.quality, None)
                    .map(|watermarked| RenderedProof {
                        clean: None,
                        watermarked,
                    })
            } else {
                let render = if proof.clean_current {
                    ProofRender::Watermarked
                } else {
                    ProofRender::All
                };

                renderer.render_proof(&original, proof.quality, proof.watermark.as_ref(), render)
            }
        })
        .await
        .map_err(HandlerError::failed)?;

        let reason = match rendered {
            Ok(rendered) => {
                return self
                    .stage(location, id, stamp, rendered)
                    .await
                    .map(PhotoOutput::Rendered);
            }
            Err(RenderError::Unreadable) => UNREADABLE_REASON,
            Err(RenderError::TooLarge) => TOO_LARGE_REASON,
            Err(error @ RenderError::Library(_)) => return Err(HandlerError::failed(error)),
        };

        warn!(photo = %id, reason, "photo cannot be processed");

        Ok(PhotoOutput::Failed { id, reason })
    }

    /// Writes the rendered pairs to temporary files. They are discarded if never published.
    async fn stage(
        &self,
        location: PhotoLocation,
        id: PhotoId,
        stamp: WatermarkStamp,
        rendered: RenderedProof,
    ) -> Result<StagedProof, HandlerError> {
        let mut derivatives = Vec::new();

        if let Some(clean) = rendered.clean {
            derivatives.extend([
                (Variant::Thumbnail, clean.thumbnail),
                (Variant::Preview, clean.preview),
            ]);
        }

        let marked = rendered.watermarked;
        let variants = if stamp.revision().is_some() {
            [Variant::WatermarkedThumbnail, Variant::WatermarkedPreview]
        } else {
            [Variant::Thumbnail, Variant::Preview]
        };

        derivatives.extend([
            (variants[0], marked.thumbnail),
            (variants[1], marked.preview),
        ]);

        let mut files = Vec::new();

        for (variant, bytes) in derivatives {
            let mut file = self.storage.stage().await.map_err(HandlerError::failed)?;

            file.write(&bytes).await.map_err(HandlerError::failed)?;
            files.push((PhotoKey::with_location(location, id, variant), file));
        }

        Ok(StagedProof { id, stamp, files })
    }

    async fn read_original(
        &self,
        location: PhotoLocation,
        id: PhotoId,
    ) -> Result<Vec<u8>, HandlerError> {
        let mut file = self
            .storage
            .read(&PhotoKey::with_location(location, id, Variant::Original))
            .await
            .map_err(HandlerError::failed)?;
        let mut original = Vec::new();

        file.read_to_end(&mut original)
            .await
            .map_err(HandlerError::failed)?;

        Ok(original)
    }
}

/// Records the result of `process_photo` jobs together with the deletion of the job.
pub struct ProcessPhotoFinalizer {
    database: Database,
    storage: PhotoStorage,
}

impl ProcessPhotoFinalizer {
    pub const fn new(database: Database, storage: PhotoStorage) -> Self {
        Self { database, storage }
    }

    /// Publishes the staged files if their watermark is still current, marking the photo ready
    /// and finishing `job` together. Only the renames happen while the job and photo are
    /// locked. If the photo was deleted meanwhile, whatever was written is removed so late or
    /// partial work leaves nothing behind; a photo that still exists keeps it, even if another
    /// run finished it.
    async fn publish(&self, job: &Job, staged: StagedProof) -> Result<JobCompletion, HandlerError> {
        let StagedProof { id, stamp, files } = staged;
        let (keys, files): (Vec<_>, Vec<_>) = files.into_iter().unzip();
        let jobs = self.database.repository::<JobRepository>();
        let published = async {
            match jobs
                .begin_publication(job, id, stamp)
                .await
                .map_err(HandlerError::failed)?
            {
                Publication::Open(open) => {
                    for (file, key) in files.into_iter().zip(&keys) {
                        file.commit(key).await.map_err(HandlerError::failed)?;
                    }

                    open.finish().await.map_err(HandlerError::failed)
                }
                Publication::Closed(completion, update) => Ok((completion, update)),
            }
        }
        .await;

        let photo_is_deleted = match &published {
            Ok((_, update)) => *update == PhotoUpdate::Missing,
            Err(_) => matches!(
                self.database
                    .repository::<PhotoRepository>()
                    .locate(id)
                    .await,
                Ok(None)
            ),
        };

        if photo_is_deleted {
            for key in &keys {
                // Best effort: a missing file is already the desired outcome.
                let _ = self.storage.remove(key).await;
            }
        }

        published.map(|(completion, _)| completion)
    }
}

impl Finalizer for ProcessPhotoFinalizer {
    type Output = PhotoOutput;

    async fn finalize(
        &self,
        job: &Job,
        output: PhotoOutput,
    ) -> Result<JobCompletion, HandlerError> {
        let jobs = self.database.repository::<JobRepository>();

        match output {
            PhotoOutput::Skipped => jobs.complete(job).await.map_err(HandlerError::failed),
            PhotoOutput::Failed { id, reason } => jobs
                .complete_photo(job, id, PhotoResult::Failed(reason))
                .await
                .map(|(completion, _)| completion)
                .map_err(HandlerError::failed),
            PhotoOutput::Rendered(staged) => self.publish(job, staged).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs, future,
        num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroUsize},
        path::{Path, PathBuf},
        time::Duration,
    };

    use libvips::ops::{gaussnoise, jpegsave_buffer};
    use pp_core::{config::PhotoDir, types::generate::Generate};
    use pp_database::{
        PgPool,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        jobs::{Backoff, JobKind, JobPayload},
        photo::{NewPhoto, PhotoDimensions, PhotoKind, PhotoStatus},
        photo_intake::PhotoIntake,
        settings::{LogoChange, ProofQuality, SettingsUpdate, WorkspaceSettings},
        tests::TestRepository,
    };
    use tempfile::TempDir;
    use tokio::time::{sleep, timeout};
    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::{
        handlers::HandlerRegistry,
        runtime::{RuntimeOptions, Worker},
    };

    struct Fixture {
        dir: TempDir,
        handler: Arc<ProcessPhotoHandler>,
        location: PhotoLocation,
        id: PhotoId,
        original: Vec<u8>,
    }

    impl Fixture {
        /// Accepts a photo with `original` stored as its upload.
        async fn new(pool: &PgPool, original: Vec<u8>) -> Self {
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
            let id = PhotoId::generate();

            database
                .repository::<PhotoIntake>()
                .try_accept(NewPhoto {
                    kind: PhotoKind::Proof,
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
                .commit(&PhotoKey::with_location(location, id, Variant::Original))
                .await
                .unwrap();

            Self {
                dir,
                handler: Arc::new(ProcessPhotoHandler::new(
                    database,
                    storage,
                    renderer(100_000_000),
                )),
                location,
                id,
                original,
            }
        }

        /// Claims the photo's job, enqueueing a new one when an earlier run finished it, and
        /// processes it. A run that finishes must still own its job.
        async fn run(&self) -> Result<(), HandlerError> {
            let job = self.claim().await;
            let output = Arc::clone(&self.handler)
                .run(ProcessPhoto { photo_id: self.id })
                .await?;
            let completion = self.finalizer().finalize(&job, output).await?;

            assert!(completion.is_current());

            Ok(())
        }

        fn finalizer(&self) -> ProcessPhotoFinalizer {
            ProcessPhotoFinalizer::new(self.handler.database.clone(), self.handler.storage.clone())
        }

        async fn stage(&self, stamp: WatermarkStamp) -> PhotoOutput {
            let rendered = self
                .handler
                .renderer
                .render_proof(
                    &self.original,
                    ProofQuality::default(),
                    None,
                    ProofRender::All,
                )
                .unwrap();

            PhotoOutput::Rendered(
                self.handler
                    .stage(self.location, self.id, stamp, rendered)
                    .await
                    .unwrap(),
            )
        }

        async fn stamp(&self) -> WatermarkStamp {
            self.handler
                .database
                .repository::<SettingsRepository>()
                .proof_watermark(self.id)
                .await
                .unwrap()
                .unwrap()
                .stamp
        }

        async fn claim(&self) -> Job {
            let jobs = self.jobs();

            if let Some(job) = jobs.claim(&[JobKind::ProcessPhoto], LEASE).await.unwrap() {
                return job;
            }

            jobs.enqueue(
                JobPayload::ProcessPhoto(ProcessPhoto { photo_id: self.id }),
                NonZeroU16::new(3).unwrap(),
            )
            .await
            .unwrap();
            jobs.claim(&[JobKind::ProcessPhoto], LEASE)
                .await
                .unwrap()
                .unwrap()
        }

        fn jobs(&self) -> JobRepository<'_> {
            self.handler.database.repository::<JobRepository>()
        }

        fn testing(&self) -> TestRepository<'_> {
            self.handler.database.repository::<TestRepository>()
        }

        /// Whether any job is still queued or running.
        async fn has_jobs(&self) -> bool {
            self.testing().count_jobs(None).await.unwrap() > 0
        }

        async fn status(&self) -> Option<PhotoStatus> {
            self.handler
                .database
                .repository::<PhotoRepository>()
                .get(
                    self.location.workspace_id,
                    self.location.gallery_id,
                    self.id,
                    PhotoKind::Proof,
                )
                .await
                .unwrap()
                .map(|photo| photo.status)
        }

        fn gallery_dir(&self) -> PathBuf {
            gallery_dir(
                self.dir.path(),
                self.location.workspace_id,
                self.location.gallery_id,
            )
        }

        fn path(&self, variant: Variant) -> PathBuf {
            self.gallery_dir().join(format!("{}-{variant}", self.id))
        }

        fn file(&self, variant: Variant) -> Option<Vec<u8>> {
            fs::read(self.path(variant)).ok()
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

    fn gallery_dir(root: &Path, workspace: WorkspaceId, gallery: GalleryId) -> PathBuf {
        root.join(workspace.0.to_string())
            .join(gallery.0.to_string())
    }

    fn renderer(max_pixels: u64) -> ImageRenderer {
        ImageRenderer::new(NonZeroU64::new(max_pixels).unwrap()).unwrap()
    }

    const LEASE: Duration = Duration::from_secs(30);

    fn worker_options() -> RuntimeOptions {
        RuntimeOptions {
            poll_interval: Duration::from_millis(30),
            lease: Duration::from_millis(600),
            concurrency: NonZeroUsize::new(1).unwrap(),
            shutdown_grace: Duration::from_millis(50),
            backoff: Backoff::new(Duration::from_millis(50), Duration::from_millis(100)),
        }
    }

    fn jpeg() -> Vec<u8> {
        crate::render::tests::init_libvips();

        jpegsave_buffer(&gaussnoise(600, 400).unwrap()).unwrap()
    }

    fn dimensions(bytes: &[u8]) -> (i32, i32) {
        let image = libvips::VipsImage::new_from_buffer(bytes, "").unwrap();

        (image.get_width(), image.get_height())
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn changing_the_mark_replaces_only_marked_files_in_a_disabled_gallery(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;

        fixture.run().await.unwrap();
        let clean = fixture.file(Variant::Preview).unwrap();
        let thumbnail = fixture.file(Variant::Thumbnail).unwrap();

        assert_eq!(fixture.file(Variant::WatermarkedPreview).unwrap(), clean);
        let defaults = WorkspaceSettings::default();

        fixture
            .handler
            .database
            .repository::<SettingsRepository>()
            .save(
                fixture.location.workspace_id,
                SettingsUpdate {
                    studio_name: "Studio".to_owned(),
                    mark: defaults.mark,
                    pattern: defaults.pattern,
                    position: defaults.position,
                    size: defaults.size,
                    signature: defaults.signature,
                    signature_size: defaults.signature_size,
                    proof_quality: defaults.proof_quality,
                    logo: LogoChange::Keep,
                },
            )
            .await
            .unwrap();
        fixture.run().await.unwrap();

        assert_eq!(fixture.file(Variant::Preview).unwrap(), clean);
        assert_eq!(fixture.file(Variant::Thumbnail).unwrap(), thumbnail);
        assert_ne!(fixture.file(Variant::WatermarkedPreview).unwrap(), clean);
        assert_ne!(
            fixture.file(Variant::WatermarkedThumbnail).unwrap(),
            thumbnail
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn processing_a_photo_publishes_both_derivatives_and_marks_it_ready(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;

        fixture.run().await.unwrap();

        assert_eq!(fixture.status().await, Some(PhotoStatus::Ready));
        assert!(!fixture.has_jobs().await);
        assert_eq!(fixture.file(Variant::Original).unwrap(), fixture.original);
        assert_eq!(
            dimensions(&fixture.file(Variant::Thumbnail).unwrap()),
            (600, 400)
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
        assert!(!fixture.has_jobs().await);
        assert_eq!(fixture.file(Variant::Thumbnail), None);
        assert_eq!(fixture.file(Variant::Preview), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn oversized_original_fails_permanently_with_a_safe_reason(pool: PgPool) {
        let mut fixture = Fixture::new(&pool, jpeg()).await;

        Arc::get_mut(&mut fixture.handler).unwrap().renderer = renderer(600 * 400 - 1);
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
        assert!(fixture.has_jobs().await);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn rerun_replaces_derivatives_without_duplicates(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;

        fixture.run().await.unwrap();
        fixture
            .testing()
            .force_photo_status(fixture.id, &PhotoStatus::Waiting)
            .await
            .unwrap();
        fixture.run().await.unwrap();

        assert_eq!(fixture.status().await, Some(PhotoStatus::Ready));
        assert_eq!(
            fixture.files(),
            [
                Variant::Original,
                Variant::Preview,
                Variant::Thumbnail,
                Variant::WatermarkedPreview,
                Variant::WatermarkedThumbnail
            ]
            .map(|variant| format!("{}-{variant}", fixture.id))
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn finished_photo_is_not_reprocessed(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let photos = fixture.handler.database.repository::<PhotoRepository>();

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

        fixture
            .testing()
            .delete_gallery(fixture.location.gallery_id)
            .await
            .unwrap();
        fixture.run().await.unwrap();

        assert_eq!(fixture.file(Variant::Thumbnail), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn late_work_for_a_deleted_photo_leaves_nothing(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let staged = fixture.stage(fixture.stamp().await).await;

        fixture
            .testing()
            .delete_gallery(fixture.location.gallery_id)
            .await
            .unwrap();

        let completion = fixture
            .finalizer()
            .finalize(&fixture.claim().await, staged)
            .await
            .unwrap();

        assert!(completion.is_current());

        assert_eq!(fixture.file(Variant::Thumbnail), None);
        assert_eq!(fixture.file(Variant::Preview), None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn deleting_a_photo_the_worker_is_publishing_leaves_no_derivative(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let job = fixture.claim().await;
        let staged = fixture.stage(fixture.stamp().await).await;

        // A lock on its job holds the publication before it goes live.
        let mut completion_gate = pool.begin().await.unwrap();

        sqlx::query(
            "SELECT 1 FROM jobs WHERE payload->'payload'->>'photo_id' = $1::text FOR UPDATE",
        )
        .bind(fixture.id)
        .execute(&mut *completion_gate)
        .await
        .unwrap();

        let finalizer = fixture.finalizer();
        let publishing = task::spawn(async move { finalizer.finalize(&job, staged).await });

        sleep(Duration::from_millis(300)).await;

        // The photo goes while its derivatives wait to be published.
        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(fixture.id)
            .execute(&pool)
            .await
            .unwrap();
        completion_gate.commit().await.unwrap();

        assert!(publishing.await.unwrap().unwrap().is_current());

        assert!(!fixture.has_jobs().await);
        assert!(fixture.file(Variant::Thumbnail).is_none());
        assert!(fixture.file(Variant::Preview).is_none());
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn late_work_for_a_ready_photo_keeps_its_derivatives(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let staged = fixture.stage(fixture.stamp().await).await;

        fixture.run().await.unwrap();

        let completion = fixture
            .finalizer()
            .finalize(&fixture.claim().await, staged)
            .await
            .unwrap();

        assert!(completion.is_current());

        assert_eq!(fixture.status().await, Some(PhotoStatus::Ready));
        assert!(fixture.file(Variant::Thumbnail).is_some());
        assert!(fixture.file(Variant::Preview).is_some());
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn worker_restart_mid_job_recovers(pool: PgPool) {
        let fixture = Fixture::new(&pool, jpeg()).await;
        let options = worker_options();

        // The first worker starts the photo, then never finishes and is stopped.
        let stuck = {
            let database = fixture.handler.database.clone();

            HandlerRegistry::default().register(
                JobKind::ProcessPhoto,
                move |payload: ProcessPhoto| {
                    let database = database.clone();

                    async move {
                        database
                            .repository::<PhotoRepository>()
                            .mark_processing(payload.photo_id)
                            .await
                            .unwrap();
                        future::pending::<Result<PhotoOutput, HandlerError>>().await
                    }
                },
                fixture.finalizer(),
            )
        };
        let shutdown = CancellationToken::new();
        let first = tokio::spawn(
            Worker::new(fixture.handler.database.clone(), stuck, options).run(shutdown.clone()),
        );

        while fixture.status().await != Some(PhotoStatus::Processing) {
            sleep(Duration::from_millis(20)).await;
        }

        shutdown.cancel();
        first.await.unwrap();

        let handler = fixture.handler.clone();
        let handlers = HandlerRegistry::default().register(
            JobKind::ProcessPhoto,
            move |payload| Arc::clone(&handler).run(payload),
            fixture.finalizer(),
        );
        let shutdown = CancellationToken::new();
        let second = tokio::spawn(
            Worker::new(fixture.handler.database.clone(), handlers, options).run(shutdown.clone()),
        );

        timeout(Duration::from_secs(10), async {
            while fixture.status().await != Some(PhotoStatus::Ready) {
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();

        shutdown.cancel();
        second.await.unwrap();
    }
}
