//! Workspace-scoped photo metadata.
//!
//! A photo is visible only through its owning workspace and gallery. [`PhotoRepository`] only
//! inserts photo metadata; [`crate::photo_intake::PhotoIntake`] coordinates that insert with its
//! processing job in one transaction.

use std::num::{NonZeroU32, NonZeroU64};

use pp_core::{notifications::PhotoId, types::timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, Row, Transaction, Type, postgres::PgRow};

use crate::{Repository, auth::WorkspaceId, error::DatabaseError, gallery::GalleryId};

/// Current server-side processing state for a photo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhotoStatus {
    Waiting,
    Processing,
    Ready,
    /// Processing failed, with the reason reported by the worker.
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "photo_status", rename_all = "snake_case")]
enum DatabasePhotoStatus {
    Waiting,
    Processing,
    Ready,
    Failed,
}

/// Header dimensions, both guaranteed to be non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhotoDimensions {
    pub width: NonZeroU32,
    pub height: NonZeroU32,
}

/// A photo exposed by the workspace-scoped photo repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Photo {
    pub id: PhotoId,
    pub original_name: String,
    pub size: NonZeroU64,
    #[serde(flatten)]
    pub dimensions: PhotoDimensions,
    pub status: PhotoStatus,
    pub created_at: Timestamp,
}

impl FromRow<'_, PgRow> for Photo {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let size = u64::try_from(row.try_get::<i64, _>("size")?)
            .ok()
            .and_then(NonZeroU64::new)
            .ok_or_else(|| sqlx::Error::Decode("photo size must be positive".into()))?;
        let width = u32::try_from(row.try_get::<i32, _>("width")?)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| sqlx::Error::Decode("photo width must be positive".into()))?;
        let height = u32::try_from(row.try_get::<i32, _>("height")?)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| sqlx::Error::Decode("photo height must be positive".into()))?;

        let database_status: DatabasePhotoStatus = row.try_get("status")?;
        let failure_reason: Option<String> = row.try_get("failure_reason")?;
        let status = match (database_status, failure_reason) {
            (DatabasePhotoStatus::Waiting, None) => PhotoStatus::Waiting,
            (DatabasePhotoStatus::Processing, None) => PhotoStatus::Processing,
            (DatabasePhotoStatus::Ready, None) => PhotoStatus::Ready,
            (DatabasePhotoStatus::Failed, Some(reason)) => PhotoStatus::Failed(reason),
            _ => {
                return Err(sqlx::Error::Decode(
                    "photo status and failure reason are inconsistent".into(),
                ));
            }
        };

        Ok(Self {
            id: row.try_get("id")?,
            original_name: row.try_get("original_name")?,
            size,
            dimensions: PhotoDimensions { width, height },
            status,
            created_at: row.try_get("created_at")?,
        })
    }
}

/// Data recorded for an accepted original photo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPhoto {
    pub id: PhotoId,
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
    pub original_name: String,
    pub size: NonZeroU64,
    pub dimensions: PhotoDimensions,
}

/// Result of inserting photo metadata within a caller-owned transaction.
///
/// Only [`Inserted`](Self::Inserted) means a job should be enqueued; an identical existing row
/// can be returned during commit reconciliation without creating a duplicate job.
///
/// # Examples
///
/// ```
/// use pp_database::photo::PhotoInsertOutcome;
///
/// fn should_enqueue_job(outcome: &PhotoInsertOutcome) -> bool {
///     matches!(outcome, PhotoInsertOutcome::Inserted(_))
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotoInsertOutcome {
    /// A new photo row was inserted.
    Inserted(Photo),
    /// An existing row has the same identifying metadata, so a retry is safe.
    Identical(Photo),
    /// The gallery does not exist in the requested workspace.
    MissingGallery,
}

/// Persistence for accepted photos and their processing jobs.
#[derive(Clone, Copy)]
pub struct PhotoRepository<'db> {
    pool: &'db PgPool,
}

impl<'db> Repository<'db> for PhotoRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl PhotoRepository<'_> {
    /// Inserts only photo metadata in the caller's transaction.
    pub async fn insert_in(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        photo: &NewPhoto,
    ) -> Result<PhotoInsertOutcome, DatabaseError> {
        let size =
            i64::try_from(photo.size.get()).map_err(|error| sqlx::Error::Encode(error.into()))?;
        let width = i32::try_from(photo.dimensions.width.get())
            .map_err(|error| sqlx::Error::Encode(error.into()))?;
        let height = i32::try_from(photo.dimensions.height.get())
            .map_err(|error| sqlx::Error::Encode(error.into()))?;
        let inserted: Option<Photo> = sqlx::query_as(
            "INSERT INTO photos
                 (id, workspace_id, gallery_id, original_name, size, width, height)
             SELECT $1, $2, g.id, $4, $5, $6, $7
             FROM galleries AS g
             WHERE g.id = $3 AND g.workspace_id = $2
             ON CONFLICT (id) DO NOTHING
             RETURNING id, original_name, size, width, height, status, failure_reason, created_at",
        )
        .bind(photo.id)
        .bind(photo.workspace_id)
        .bind(photo.gallery_id)
        .bind(&photo.original_name)
        .bind(size)
        .bind(width)
        .bind(height)
        .fetch_optional(&mut **transaction)
        .await?;
        if let Some(inserted) = inserted {
            return Ok(PhotoInsertOutcome::Inserted(inserted));
        }

        let existing = sqlx::query(
            "SELECT id, workspace_id, gallery_id, original_name, size, width, height,
                    status, failure_reason, created_at
             FROM photos WHERE id = $1",
        )
        .bind(photo.id)
        .fetch_optional(&mut **transaction)
        .await?;
        let Some(row) = existing else {
            return Ok(PhotoInsertOutcome::MissingGallery);
        };
        let existing = Photo::from_row(&row)?;
        let identical = row.try_get::<WorkspaceId, _>("workspace_id")? == photo.workspace_id
            && row.try_get::<GalleryId, _>("gallery_id")? == photo.gallery_id
            && existing.original_name == photo.original_name
            && existing.size == photo.size
            && existing.dimensions == photo.dimensions;
        if !identical {
            return Err(DatabaseError::Other(
                "photo id collision with different metadata".to_owned(),
            ));
        }

        Ok(PhotoInsertOutcome::Identical(existing))
    }

    /// Lists a gallery's photos in creation order, scoped to the owning workspace.
    pub async fn list(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
    ) -> Result<Vec<Photo>, DatabaseError> {
        Ok(sqlx::query_as(
            "SELECT id, original_name, size, width, height, status, failure_reason, created_at
             FROM photos WHERE workspace_id = $1 AND gallery_id = $2
             ORDER BY created_at, id",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .fetch_all(self.pool)
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};

    use pp_core::notifications::PhotoId;
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        photo_intake::PhotoIntake,
    };

    async fn workspace(pool: &PgPool) -> WorkspaceId {
        let id = WorkspaceId(Uuid::new_v4());

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();

        id
    }

    fn new_photo(
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        id: PhotoId,
        original_name: &str,
    ) -> NewPhoto {
        NewPhoto {
            id,
            workspace_id,
            gallery_id,
            original_name: original_name.to_owned(),
            size: NonZeroU64::new(1024).unwrap(),
            dimensions: PhotoDimensions {
                width: NonZeroU32::new(20).unwrap(),
                height: NonZeroU32::new(10).unwrap(),
            },
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn listing_is_creation_ordered_and_keeps_duplicate_names_distinct(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photos = database.repository::<PhotoRepository>();
        let intake = database.repository::<PhotoIntake>();
        let mut expected = Vec::new();

        for (created_at, status, failure_reason, original_name) in [
            (30_i64, "ready", None, "same.jpg"),
            (10, "waiting", None, "same.jpg"),
            (40, "failed", Some("processing failed"), "third.jpg"),
            (20, "processing", None, "fourth.jpg"),
        ] {
            let id = PhotoId::from(Uuid::new_v4());
            intake
                .accept(new_photo(workspace_id, gallery.id, id, original_name))
                .await
                .unwrap()
                .unwrap();
            sqlx::query(
                "UPDATE photos SET status = $2::photo_status, failure_reason = $3,
                 created_at = to_timestamp($4)
                 WHERE id = $1",
            )
            .bind(id)
            .bind(status)
            .bind(failure_reason)
            .bind(created_at)
            .execute(&pool)
            .await
            .unwrap();
            expected.push((created_at, id, status, failure_reason, original_name));
        }
        expected.sort_by_key(|(created_at, ..)| *created_at);

        let listed = photos.list(workspace_id, gallery.id).await.unwrap();

        assert_eq!(
            listed.iter().map(|photo| photo.id).collect::<Vec<_>>(),
            expected.iter().map(|(_, id, ..)| *id).collect::<Vec<_>>()
        );
        assert_eq!(
            listed
                .iter()
                .map(|photo| photo.status.clone())
                .collect::<Vec<_>>(),
            [
                PhotoStatus::Waiting,
                PhotoStatus::Processing,
                PhotoStatus::Ready,
                PhotoStatus::Failed("processing failed".to_owned()),
            ]
        );
        assert_eq!(listed[0].original_name, "same.jpg");
        assert_eq!(listed[2].original_name, "same.jpg");
        assert_ne!(listed[0].id, listed[2].id);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn database_requires_failure_reason_exactly_for_failed_photos(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photo_id = PhotoId::from(Uuid::new_v4());

        database
            .repository::<PhotoIntake>()
            .accept(new_photo(workspace_id, gallery.id, photo_id, "proof.jpg"))
            .await
            .unwrap()
            .unwrap();

        let failed_without_reason =
            sqlx::query("UPDATE photos SET status = 'failed', failure_reason = NULL WHERE id = $1")
                .bind(photo_id)
                .execute(&pool)
                .await;
        let ready_with_reason = sqlx::query(
            "UPDATE photos SET status = 'ready', failure_reason = 'not failed' WHERE id = $1",
        )
        .bind(photo_id)
        .execute(&pool)
        .await;

        assert!(failed_without_reason.is_err());
        assert!(ready_with_reason.is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn another_workspace_cannot_list_or_create_photos_in_a_gallery(pool: PgPool) {
        let database = Database::new(pool.clone());
        let (owner, other) = (workspace(&pool).await, workspace(&pool).await);
        let gallery = database
            .repository::<GalleryRepository>()
            .create(owner)
            .await
            .unwrap();
        let photos = database.repository::<PhotoRepository>();
        let intake = database.repository::<PhotoIntake>();

        assert!(photos.list(other, gallery.id).await.unwrap().is_empty());
        assert!(
            intake
                .accept(new_photo(
                    other,
                    gallery.id,
                    PhotoId::from(Uuid::new_v4()),
                    "foreign.jpg",
                ))
                .await
                .unwrap()
                .is_none()
        );
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(count, 0);
    }
}
