//! Requeues photos whose processing failed for good.

use pp_core::notifications::PhotoId;
use sqlx::PgPool;

use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::GalleryId,
    jobs::{JobOptions, JobPayload, JobRepository, ProcessPhoto},
    photo::PhotoKind,
};

/// Resets failed photos to waiting and gives each one a fresh processing job.
pub struct PhotoRetry<'db> {
    pool: &'db PgPool,
    jobs: JobOptions,
}

impl<'db> Repository<'db> for PhotoRetry<'db> {
    type Options = JobOptions;

    fn open(pool: &'db PgPool, jobs: Self::Options) -> Self {
        Self { pool, jobs }
    }
}

impl PhotoRetry<'_> {
    /// Requeues the gallery's failed photos of one kind, or only `photo` when given.
    ///
    /// Returns `None` when the gallery, or the given photo within it, does not exist in the
    /// workspace, otherwise the number of photos requeued; photos that are not failed are left
    /// alone. Locks the workspace before jobs/photos so retries cannot start against a
    /// settings change that skipped their failed rows.
    pub async fn retry_failed(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        kind: PhotoKind,
        photo: Option<PhotoId>,
    ) -> Result<Option<u64>, DatabaseError> {
        let mut transaction = self.pool.begin().await?;

        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR KEY SHARE")
            .bind(workspace_id)
            .fetch_optional(&mut *transaction)
            .await?;

        let target_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM galleries WHERE id = $2 AND workspace_id = $1)
                AND ($3::uuid IS NULL OR EXISTS(
                    SELECT 1 FROM photos
                    WHERE id = $3 AND gallery_id = $2 AND workspace_id = $1
                      AND kind = $4 AND removed_at IS NULL))",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(photo)
        .bind(kind)
        .fetch_one(&mut *transaction)
        .await?;

        if !target_exists {
            return Ok(None);
        }

        let failed_photo_ids: Vec<PhotoId> = sqlx::query_scalar(
            "SELECT id FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND status = 'failed'
               AND kind = $4 AND removed_at IS NULL AND ($3::uuid IS NULL OR id = $3)",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(photo)
        .bind(kind)
        .fetch_all(&mut *transaction)
        .await?;
        let failed_photo_keys = failed_photo_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        // Jobs are locked before photos, each in id order, like the worker and delete do;
        // otherwise a concurrent delete deadlocks. Only the photos read above are touched, so a
        // photo failing meanwhile is never updated without its jobs locked. A concurrent retry
        // waits, then finds the photos no longer failed.
        sqlx::query(
            "SELECT id FROM jobs WHERE payload->'payload'->>'photo_id' = ANY($1)
             ORDER BY id
             FOR UPDATE",
        )
        .bind(&failed_photo_keys)
        .execute(&mut *transaction)
        .await?;

        let requeued_photo_ids: Vec<PhotoId> = sqlx::query_scalar(
            "UPDATE photos SET status = 'waiting', failure_reason = NULL
             WHERE id IN (
                 SELECT id FROM photos
                 WHERE id = ANY($1) AND status = 'failed'
                 ORDER BY id
                 FOR UPDATE)
             RETURNING id",
        )
        .bind(&failed_photo_ids)
        .fetch_all(&mut *transaction)
        .await?;
        let photo_ids = requeued_photo_ids
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();

        sqlx::query(
            "DELETE FROM jobs
             WHERE state = 'failed' AND payload->'payload'->>'photo_id' = ANY($1)",
        )
        .bind(photo_ids)
        .execute(&mut *transaction)
        .await?;

        for &photo_id in &requeued_photo_ids {
            let payload = JobPayload::ProcessPhoto(ProcessPhoto { photo_id });

            JobRepository::enqueue_in(&mut transaction, payload, self.jobs.max_attempts).await?;
        }

        transaction.commit().await?;

        Ok(Some(requeued_photo_ids.len() as u64))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        num::{NonZeroU32, NonZeroU64},
        time::Duration,
    };

    use pp_core::types::generate::Generate;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        gallery::GalleryRepository,
        jobs::{JobKind, JobRepository, JobState},
        photo::{NewPhoto, PhotoDimensions, PhotoKind, PhotoRepository, PhotoStatus},
        photo_intake::PhotoIntake,
        tests::TestRepository,
    };

    struct Gallery {
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
    }

    async fn gallery(database: &Database, pool: &PgPool) -> Gallery {
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

        Gallery {
            workspace_id,
            gallery_id: gallery.id,
        }
    }

    async fn photo(database: &Database, gallery: &Gallery) -> PhotoId {
        let id = PhotoId::generate();

        database
            .repository::<PhotoIntake>()
            .try_accept(NewPhoto {
                kind: PhotoKind::Proof,
                id,
                workspace_id: gallery.workspace_id,
                gallery_id: gallery.gallery_id,
                original_name: "proof.jpg".to_owned(),
                size: NonZeroU64::new(1024).unwrap(),
                dimensions: PhotoDimensions {
                    width: NonZeroU32::new(20).unwrap(),
                    height: NonZeroU32::new(10).unwrap(),
                },
            })
            .await
            .unwrap()
            .unwrap();

        id
    }

    /// Accepts a photo whose only job then failed for good.
    async fn failed_photo(database: &Database, pool: &PgPool, gallery: &Gallery) -> PhotoId {
        let id = photo(database, gallery).await;

        fail(database, pool, id).await;

        id
    }

    /// Fails the photo's job and the photo for good.
    async fn fail(database: &Database, pool: &PgPool, id: PhotoId) {
        sqlx::query(
            "UPDATE jobs SET state = 'failed', last_error = 'boom'
             WHERE payload->'payload'->>'photo_id' = $1",
        )
        .bind(id.to_string())
        .execute(pool)
        .await
        .unwrap();
        database
            .repository::<PhotoRepository>()
            .mark_failed(id, "Processing failed after several attempts.")
            .await
            .unwrap();
    }

    async fn status(database: &Database, gallery: &Gallery, id: PhotoId) -> PhotoStatus {
        database
            .repository::<PhotoRepository>()
            .get(
                gallery.workspace_id,
                gallery.gallery_id,
                id,
                PhotoKind::Proof,
            )
            .await
            .unwrap()
            .unwrap()
            .status
    }

    /// States of every job targeting `id`.
    async fn job_states(pool: &PgPool, id: PhotoId) -> Vec<JobState> {
        sqlx::query_scalar("SELECT state FROM jobs WHERE payload->'payload'->>'photo_id' = $1")
            .bind(id.to_string())
            .fetch_all(pool)
            .await
            .unwrap()
    }

    async fn retry(database: &Database, gallery: &Gallery, photo: Option<PhotoId>) -> Option<u64> {
        database
            .repository::<PhotoRetry>()
            .retry_failed(
                gallery.workspace_id,
                gallery.gallery_id,
                PhotoKind::Proof,
                photo,
            )
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_requeues_every_failed_photo_in_the_gallery(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let first = failed_photo(&database, &pool, &gallery).await;
        let second = failed_photo(&database, &pool, &gallery).await;

        assert_eq!(retry(&database, &gallery, None).await, Some(2));

        for id in [first, second] {
            assert_eq!(status(&database, &gallery, id).await, PhotoStatus::Waiting);
            assert_eq!(job_states(&pool, id).await, [JobState::Queued]);
        }

        let job = database
            .repository::<JobRepository>()
            .claim(&[JobKind::ProcessPhoto], Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(job.attempts, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retried_photos_get_the_configured_job_attempts(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let id = failed_photo(&database, &pool, &gallery).await;
        let options = JobOptions {
            max_attempts: std::num::NonZeroU16::new(9).unwrap(),
            ..JobOptions::default()
        };

        database
            .repository_with::<PhotoRetry>(options)
            .retry_failed(
                gallery.workspace_id,
                gallery.gallery_id,
                PhotoKind::Proof,
                None,
            )
            .await
            .unwrap();

        let attempts: i32 = sqlx::query_scalar(
            "SELECT max_attempts FROM jobs
             WHERE state = 'queued' AND payload->'payload'->>'photo_id' = $1",
        )
        .bind(id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(attempts, 9);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_one_photo_leaves_other_failed_photos_failed(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let retried = failed_photo(&database, &pool, &gallery).await;
        let untouched = failed_photo(&database, &pool, &gallery).await;

        assert_eq!(retry(&database, &gallery, Some(retried)).await, Some(1));
        assert_eq!(
            status(&database, &gallery, retried).await,
            PhotoStatus::Waiting
        );
        assert!(matches!(
            status(&database, &gallery, untouched).await,
            PhotoStatus::Failed(_)
        ));
        assert_eq!(job_states(&pool, untouched).await, [JobState::Failed]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_locks_jobs_before_photos_like_delete(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let id = failed_photo(&database, &pool, &gallery).await;
        let mut deleting = pool.begin().await.unwrap();

        // Deleting a photo locks its jobs first, then the photo.
        sqlx::query("SELECT id FROM jobs WHERE payload->'payload'->>'photo_id' = $1 FOR UPDATE")
            .bind(id.to_string())
            .execute(&mut *deleting)
            .await
            .unwrap();

        let retrying = tokio::spawn({
            let database = database.clone();
            let (workspace_id, gallery_id) = (gallery.workspace_id, gallery.gallery_id);

            async move {
                database
                    .repository::<PhotoRetry>()
                    .retry_failed(workspace_id, gallery_id, PhotoKind::Proof, Some(id))
                    .await
            }
        });

        database
            .repository::<TestRepository>()
            .wait_for_a_lock_waiter()
            .await
            .unwrap();

        sqlx::query("UPDATE photos SET width = width WHERE id = $1")
            .bind(id)
            .execute(&mut *deleting)
            .await
            .unwrap();
        deleting.commit().await.unwrap();

        assert_eq!(retrying.await.unwrap().unwrap(), Some(1));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_requeues_only_the_failed_photos_whose_jobs_it_locked(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let locked = failed_photo(&database, &pool, &gallery).await;
        let failing_meanwhile = photo(&database, &gallery).await;
        let mut lock_holder = pool.begin().await.unwrap();

        sqlx::query("SELECT id FROM jobs WHERE payload->'payload'->>'photo_id' = $1 FOR UPDATE")
            .bind(locked.to_string())
            .execute(&mut *lock_holder)
            .await
            .unwrap();

        let retrying = tokio::spawn({
            let database = database.clone();
            let (workspace_id, gallery_id) = (gallery.workspace_id, gallery.gallery_id);

            async move {
                database
                    .repository::<PhotoRetry>()
                    .retry_failed(workspace_id, gallery_id, PhotoKind::Proof, None)
                    .await
            }
        });

        database
            .repository::<TestRepository>()
            .wait_for_a_lock_waiter()
            .await
            .unwrap();

        fail(&database, &pool, failing_meanwhile).await;
        lock_holder.commit().await.unwrap();

        assert_eq!(retrying.await.unwrap().unwrap(), Some(1));
        assert_eq!(
            status(&database, &gallery, locked).await,
            PhotoStatus::Waiting
        );
        assert!(matches!(
            status(&database, &gallery, failing_meanwhile).await,
            PhotoStatus::Failed(_)
        ));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_without_a_failed_job_still_enqueues_one(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let id = failed_photo(&database, &pool, &gallery).await;

        sqlx::query("DELETE FROM jobs")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(retry(&database, &gallery, Some(id)).await, Some(1));
        assert_eq!(job_states(&pool, id).await, [JobState::Queued]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_replaces_an_existing_failed_job(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let id = failed_photo(&database, &pool, &gallery).await;

        assert_eq!(job_states(&pool, id).await, [JobState::Failed]);
        assert_eq!(retry(&database, &gallery, Some(id)).await, Some(1));
        assert_eq!(job_states(&pool, id).await, [JobState::Queued]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_ignores_waiting_processing_and_ready_photos(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let photos = database.repository::<PhotoRepository>();
        let waiting = photo(&database, &gallery).await;
        let processing = photo(&database, &gallery).await;
        let ready = photo(&database, &gallery).await;

        assert!(photos.mark_processing(processing).await.unwrap());
        assert!(photos.mark_ready(ready).await.unwrap());
        assert_eq!(retry(&database, &gallery, None).await, Some(0));
        assert_eq!(retry(&database, &gallery, Some(ready)).await, Some(0));

        for (id, expected) in [
            (waiting, PhotoStatus::Waiting),
            (processing, PhotoStatus::Processing),
            (ready, PhotoStatus::Ready),
        ] {
            assert_eq!(status(&database, &gallery, id).await, expected);
            assert_eq!(job_states(&pool, id).await, [JobState::Queued]);
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn repeating_retry_does_not_duplicate_jobs(pool: PgPool) {
        let database = Database::new(pool.clone());
        let gallery = gallery(&database, &pool).await;
        let id = failed_photo(&database, &pool, &gallery).await;

        assert_eq!(retry(&database, &gallery, None).await, Some(1));
        assert_eq!(retry(&database, &gallery, None).await, Some(0));
        assert_eq!(job_states(&pool, id).await, [JobState::Queued]);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_in_a_foreign_workspace_gallery_or_unknown_photo_returns_none(pool: PgPool) {
        let database = Database::new(pool.clone());
        let owner = gallery(&database, &pool).await;
        let other = gallery(&database, &pool).await;
        let id = failed_photo(&database, &pool, &owner).await;
        let foreign_workspace = Gallery {
            workspace_id: other.workspace_id,
            gallery_id: owner.gallery_id,
        };
        let missing_gallery = Gallery {
            workspace_id: owner.workspace_id,
            gallery_id: GalleryId::generate(),
        };

        assert_eq!(retry(&database, &foreign_workspace, Some(id)).await, None);
        assert_eq!(retry(&database, &missing_gallery, None).await, None);
        assert_eq!(retry(&database, &other, Some(id)).await, None);
        assert_eq!(
            retry(&database, &owner, Some(PhotoId::generate())).await,
            None
        );
        assert!(matches!(
            status(&database, &owner, id).await,
            PhotoStatus::Failed(_)
        ));
    }
}
