//! Deletes galleries and photos together with the jobs that remove their files.

use sqlx::PgPool;

use crate::photo::PhotoId;
use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    export::ExportRepository,
    gallery::GalleryId,
    jobs::{CleanupGallery, CleanupPhotos, JobOptions, JobRepository},
    photo::PhotoKind,
};

/// Removes rows and queues the cleanup of their files in one transaction, so neither can happen
/// without the other.
pub struct WorkspaceDeletion<'db> {
    pool: &'db PgPool,
    options: JobOptions,
}

impl<'db> Repository<'db> for WorkspaceDeletion<'db> {
    type Options = JobOptions;

    fn open(pool: &'db PgPool, options: Self::Options) -> Self {
        Self { pool, options }
    }
}

impl WorkspaceDeletion<'_> {
    /// Deletes the gallery with its photos and favorites, and queues the removal of its files.
    /// `false` if the workspace has no such gallery.
    ///
    /// A worker still rendering one of its photos finds its claim gone and removes what it
    /// writes late.
    pub async fn delete_gallery(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<bool, DatabaseError> {
        let mut transaction = self.pool.begin().await?;

        let photo_ids: Vec<PhotoId> = sqlx::query_scalar(
            "SELECT p.id FROM photos p
             JOIN galleries g ON g.id = p.gallery_id
             WHERE g.id = $1 AND g.workspace_id = $2",
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_all(&mut *transaction)
        .await?;

        JobRepository::delete_photo_jobs_in(&mut transaction, &photo_ids).await?;
        ExportRepository::delete_exports_in(&mut transaction, id).await?;

        let removed_galleries =
            sqlx::query("DELETE FROM galleries WHERE id=$1 AND workspace_id=$2")
                .bind(id)
                .bind(workspace_id)
                .execute(&mut *transaction)
                .await?;

        if removed_galleries.rows_affected() == 0 {
            return Ok(false);
        }

        JobRepository::enqueue_in(
            &mut transaction,
            CleanupGallery {
                workspace_id,
                gallery_id: id,
            },
            self.options.max_attempts,
        )
        .await?;
        transaction.commit().await?;

        Ok(true)
    }

    /// Deletes photos of one kind whatever their status, together with all of their processing
    /// jobs, and queues the removal of their files.
    ///
    /// Returns the ids removed, each once; ids outside this workspace, gallery and kind, or
    /// already gone, are ignored. A worker still rendering one finds its claim gone and removes
    /// whatever it writes late.
    pub async fn delete_photos(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        kind: PhotoKind,
        ids: &[PhotoId],
    ) -> Result<Vec<PhotoId>, DatabaseError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut transaction = self.pool.begin().await?;

        let photo_ids: Vec<PhotoId> = sqlx::query_scalar(
            "SELECT id FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = ANY($3)
               AND kind = $4",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(ids)
        .bind(kind)
        .fetch_all(&mut *transaction)
        .await?;

        JobRepository::delete_photo_jobs_in(&mut transaction, &photo_ids).await?;

        let deleted_ids: Vec<PhotoId> = sqlx::query_scalar(
            "DELETE FROM photos WHERE id IN (
                 SELECT id FROM photos
                 WHERE workspace_id = $1 AND gallery_id = $2 AND id = ANY($3)
                   AND kind = $4
                 ORDER BY id FOR UPDATE
             )
             RETURNING id",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(ids)
        .bind(kind)
        .fetch_all(&mut *transaction)
        .await?;

        if deleted_ids.is_empty() {
            return Ok(deleted_ids);
        }

        for batch in deleted_ids.chunks(self.options.cleanup_batch.get()) {
            JobRepository::enqueue_in(
                &mut transaction,
                CleanupPhotos {
                    workspace_id,
                    gallery_id,
                    photo_ids: batch.to_vec(),
                },
                self.options.max_attempts,
            )
            .await?;
        }

        transaction.commit().await?;

        Ok(deleted_ids)
    }
}
