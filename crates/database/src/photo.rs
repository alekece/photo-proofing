//! Workspace-scoped photo metadata and atomic photo/job creation.
//!
//! A photo is visible only through its owning workspace and gallery. Accepting one writes its
//! metadata and `process_photo` job in the same transaction, so the queue cannot observe a
//! partially-created photo.

use std::num::{NonZeroU32, NonZeroU64};

use pp_core::{notifications::PhotoId, types::timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row, Type, postgres::PgRow};

use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::GalleryId,
    jobs::{DEFAULT_MAX_ATTEMPTS, JobPayload, JobRepository},
};

/// Current server-side processing state for a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "photo_status", rename_all = "snake_case")]
pub enum PhotoStatus {
    Waiting,
    Processing,
    Ready,
    Failed,
}

/// Original file size, guaranteed to be non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PhotoSize(NonZeroU64);

impl PhotoSize {
    pub const fn new(bytes: NonZeroU64) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> NonZeroU64 {
        self.0
    }
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
    pub size: PhotoSize,
    #[serde(flatten)]
    pub dimensions: PhotoDimensions,
    pub status: PhotoStatus,
    pub failure_reason: Option<String>,
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

        Ok(Self {
            id: row.try_get("id")?,
            original_name: row.try_get("original_name")?,
            size: PhotoSize::new(size),
            dimensions: PhotoDimensions { width, height },
            status: row.try_get("status")?,
            failure_reason: row.try_get("failure_reason")?,
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
    pub size: PhotoSize,
    pub dimensions: PhotoDimensions,
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
    /// Stores photo metadata and its processing job atomically.
    ///
    /// Returns `None` when the gallery is missing or not owned by `workspace_id`.
    pub async fn create_with_job(&self, photo: NewPhoto) -> Result<Option<Photo>, DatabaseError> {
        let size = i64::try_from(photo.size.bytes().get())
            .map_err(|error| sqlx::Error::Encode(error.into()))?;
        let width = i32::try_from(photo.dimensions.width.get())
            .map_err(|error| sqlx::Error::Encode(error.into()))?;
        let height = i32::try_from(photo.dimensions.height.get())
            .map_err(|error| sqlx::Error::Encode(error.into()))?;
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query_as(
            "INSERT INTO photos
                 (id, workspace_id, gallery_id, original_name, size, width, height)
             SELECT $1, $2, g.id, $4, $5, $6, $7
             FROM galleries AS g
             WHERE g.id = $3 AND g.workspace_id = $2
             RETURNING id, original_name, size, width, height, status, failure_reason, created_at",
        )
        .bind(photo.id)
        .bind(photo.workspace_id)
        .bind(photo.gallery_id)
        .bind(photo.original_name)
        .bind(size)
        .bind(width)
        .bind(height)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(inserted) = inserted else {
            transaction.rollback().await?;

            return Ok(None);
        };
        let payload = JobPayload::ProcessPhoto(serde_json::json!({"photo_id": photo.id}));

        JobRepository::enqueue_in(&mut transaction, payload, DEFAULT_MAX_ATTEMPTS).await?;
        transaction.commit().await?;

        Ok(Some(inserted))
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
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
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
            size: PhotoSize::new(NonZeroU64::new(1024).unwrap()),
            dimensions: PhotoDimensions {
                width: NonZeroU32::new(20).unwrap(),
                height: NonZeroU32::new(10).unwrap(),
            },
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn creating_a_photo_persists_its_processing_job(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photo_id = PhotoId::from(Uuid::new_v4());

        let photo = database
            .repository::<PhotoRepository>()
            .create_with_job(new_photo(workspace_id, gallery.id, photo_id, "proof.jpg"))
            .await
            .unwrap()
            .unwrap();
        let payload: serde_json::Value = sqlx::query_scalar(
            "SELECT payload FROM jobs WHERE payload->'payload'->>'photo_id' = $1",
        )
        .bind(photo_id.to_string())
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(photo.id, photo_id);
        assert_eq!(
            payload,
            json!({"kind": "process_photo", "payload": {"photo_id": photo_id.to_string()}})
        );
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
        let mut expected = Vec::new();

        for (created_at, status, original_name) in [
            (30_i64, "ready", "same.jpg"),
            (10, "waiting", "same.jpg"),
            (40, "failed", "third.jpg"),
            (20, "processing", "fourth.jpg"),
        ] {
            let id = PhotoId::from(Uuid::new_v4());
            photos
                .create_with_job(new_photo(workspace_id, gallery.id, id, original_name))
                .await
                .unwrap()
                .unwrap();
            sqlx::query(
                "UPDATE photos SET status = $2::photo_status, created_at = to_timestamp($3)
                 WHERE id = $1",
            )
            .bind(id)
            .bind(status)
            .bind(created_at)
            .execute(&pool)
            .await
            .unwrap();
            expected.push((created_at, id, status, original_name));
        }
        expected.sort_by_key(|(created_at, ..)| *created_at);

        let listed = photos.list(workspace_id, gallery.id).await.unwrap();

        assert_eq!(
            listed.iter().map(|photo| photo.id).collect::<Vec<_>>(),
            expected.iter().map(|(_, id, ..)| *id).collect::<Vec<_>>()
        );
        assert_eq!(
            listed.iter().map(|photo| photo.status).collect::<Vec<_>>(),
            [
                PhotoStatus::Waiting,
                PhotoStatus::Processing,
                PhotoStatus::Ready,
                PhotoStatus::Failed,
            ]
        );
        assert_eq!(listed[0].original_name, "same.jpg");
        assert_eq!(listed[2].original_name, "same.jpg");
        assert_ne!(listed[0].id, listed[2].id);
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

        assert!(photos.list(other, gallery.id).await.unwrap().is_empty());
        assert!(
            photos
                .create_with_job(new_photo(
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

    #[sqlx::test(migrations = "./migrations")]
    async fn photo_creation_rolls_back_when_job_insertion_fails(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        sqlx::query(
            "CREATE FUNCTION reject_job_insert() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RAISE EXCEPTION 'injected test failure'; END; $$",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_job_insert BEFORE INSERT ON jobs
             FOR EACH ROW EXECUTE FUNCTION reject_job_insert()",
        )
        .execute(&pool)
        .await
        .unwrap();

        let result = database
            .repository::<PhotoRepository>()
            .create_with_job(new_photo(
                workspace_id,
                gallery.id,
                PhotoId::from(Uuid::new_v4()),
                "proof.jpg",
            ))
            .await;

        assert!(result.is_err());
        let photo_count: i64 = sqlx::query_scalar("SELECT count(*) FROM photos")
            .fetch_one(&pool)
            .await
            .unwrap();
        let job_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(photo_count, 0);
        assert_eq!(job_count, 0);
    }
}
