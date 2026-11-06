//! Deletes galleries and photos together with the jobs that remove their files.

use std::num::NonZeroUsize;

use pp_core::notifications::PhotoId;
use sqlx::PgPool;

use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::GalleryId,
    jobs::{CleanupGallery, CleanupPhotos, JobOptions, JobRepository},
};

/// Photos one cleanup job names unless configured otherwise.
pub const DEFAULT_CLEANUP_BATCH: NonZeroUsize = NonZeroUsize::new(500).unwrap();

/// How deletions queue the removal of files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeletionOptions {
    pub jobs: JobOptions,
    /// Most photos one cleanup job names, so a bulk delete never produces an oversized job.
    pub cleanup_batch: NonZeroUsize,
}

impl Default for DeletionOptions {
    fn default() -> Self {
        Self {
            jobs: JobOptions::default(),
            cleanup_batch: DEFAULT_CLEANUP_BATCH,
        }
    }
}

/// Removes rows and queues the cleanup of their files in one transaction, so neither can happen
/// without the other.
pub struct Deletion<'db> {
    pool: &'db PgPool,
    options: DeletionOptions,
}

impl<'db> Repository<'db> for Deletion<'db> {
    type Options = DeletionOptions;

    fn open(pool: &'db PgPool, options: Self::Options) -> Self {
        Self { pool, options }
    }
}

impl Deletion<'_> {
    /// Deletes the gallery with its photos and favorites, and queues the removal of its files.
    /// `false` if the workspace has no such gallery.
    ///
    /// A worker still rendering one of its photos finds its claim gone and removes what it
    /// writes late.
    pub async fn gallery(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<bool, DatabaseError> {
        let mut transaction = self.pool.begin().await?;

        // Jobs before the gallery, in id order, like a photo delete (see `Self::photos`).
        sqlx::query(
            "DELETE FROM jobs WHERE id IN (
                 SELECT id FROM jobs
                 WHERE payload->>'kind' = 'process_photo'
                   AND payload->'payload'->>'photo_id' IN (
                       SELECT p.id::text FROM photos p
                       JOIN galleries g ON g.id = p.gallery_id
                       WHERE g.id = $1 AND g.workspace_id = $2)
                 ORDER BY id FOR UPDATE
             )",
        )
        .bind(id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;

        let removal = sqlx::query("DELETE FROM galleries WHERE id=$1 AND workspace_id=$2")
            .bind(id)
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await?;

        if removal.rows_affected() == 0 {
            return Ok(false);
        }

        JobRepository::enqueue_in(
            &mut transaction,
            CleanupGallery {
                workspace_id,
                gallery_id: id,
            },
            self.options.jobs.max_attempts,
        )
        .await?;
        transaction.commit().await?;

        Ok(true)
    }

    /// Deletes photos whatever their status, together with all of their processing jobs, and
    /// queues the removal of their files.
    ///
    /// Returns the ids removed, each once; ids outside this workspace and gallery, or already
    /// gone, are ignored and never named by a cleanup job. A worker still rendering one finds
    /// its claim gone and removes whatever it writes late.
    pub async fn photos(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        ids: &[PhotoId],
    ) -> Result<Vec<PhotoId>, DatabaseError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut transaction = self.pool.begin().await?;

        // Jobs before photos, each in id order: the worker locks a job then its photo, so neither
        // it nor an overlapping delete deadlocks with this one.
        sqlx::query(
            "DELETE FROM jobs WHERE id IN (
                 SELECT id FROM jobs
                 WHERE payload->>'kind' = 'process_photo'
                   AND payload->'payload'->>'photo_id' IN (
                     SELECT id::text FROM photos
                     WHERE workspace_id = $1 AND gallery_id = $2 AND id = ANY($3)
                 )
                 ORDER BY id FOR UPDATE
             )",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(ids)
        .execute(&mut *transaction)
        .await?;

        let removed_ids: Vec<PhotoId> = sqlx::query_scalar(
            "DELETE FROM photos WHERE id IN (
                 SELECT id FROM photos
                 WHERE workspace_id = $1 AND gallery_id = $2 AND id = ANY($3)
                 ORDER BY id FOR UPDATE
             )
             RETURNING id",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(ids)
        .fetch_all(&mut *transaction)
        .await?;

        if removed_ids.is_empty() {
            return Ok(removed_ids);
        }

        for batch in removed_ids.chunks(self.options.cleanup_batch.get()) {
            JobRepository::enqueue_in(
                &mut transaction,
                CleanupPhotos {
                    workspace_id,
                    gallery_id,
                    photo_ids: batch.to_vec(),
                },
                self.options.jobs.max_attempts,
            )
            .await?;
        }

        transaction.commit().await?;

        Ok(removed_ids)
    }
}
