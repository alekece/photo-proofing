//! Workspace-scoped photo metadata.
//!
//! A photo is visible only through its owning workspace and gallery. [`PhotoRepository`] stores
//! photo metadata and its processing status; [`crate::photo_intake::PhotoIntake`] coordinates the
//! insert with its processing job in one transaction.

use std::num::{NonZeroU32, NonZeroU64};

use pp_core::{notifications::PhotoId, types::timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{
    AssertSqlSafe, FromRow, PgExecutor, PgPool, Postgres, Row, Transaction, postgres::PgRow,
};

use crate::{Repository, auth::WorkspaceId, error::DatabaseError, gallery::GalleryId};

/// Reason shown when the worker gives up on a photo after its last attempt.
pub const PROCESSING_FAILED_REASON: &str = "Processing failed after several attempts.";

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

/// Displayed dimensions (after the EXIF orientation), both non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhotoDimensions {
    pub width: NonZeroU32,
    pub height: NonZeroU32,
}

impl FromRow<'_, PgRow> for PhotoDimensions {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let width = u32::try_from(row.try_get::<i32, _>("width")?)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| sqlx::Error::Decode("photo width must be positive".into()))?;
        let height = u32::try_from(row.try_get::<i32, _>("height")?)
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| sqlx::Error::Decode("photo height must be positive".into()))?;

        Ok(Self { width, height })
    }
}

/// A photo a client may see: ready, with the size it displays at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct ReadyPhoto {
    pub id: PhotoId,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub dimensions: PhotoDimensions,
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
        let dimensions = PhotoDimensions::from_row(row)?;
        let database_status: String = row.try_get("status")?;
        let failure_reason: Option<String> = row.try_get("failure_reason")?;
        let status = match (database_status.as_str(), failure_reason) {
            ("waiting", None) => PhotoStatus::Waiting,
            ("processing", None) => PhotoStatus::Processing,
            ("ready", None) => PhotoStatus::Ready,
            ("failed", Some(reason)) => PhotoStatus::Failed(reason),
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
            dimensions,
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

/// Owning workspace and gallery of a photo, used to build its storage keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct PhotoLocation {
    pub workspace_id: WorkspaceId,
    pub gallery_id: GalleryId,
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
    const COLUMNS: &'static str = "id, original_name, size, width, height, status::text AS status, failure_reason, created_at";

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
             RETURNING id, original_name, size, width, height,
                       status::text AS status, failure_reason, created_at",
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
                    status::text AS status, failure_reason, created_at
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
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2
             ORDER BY created_at, id",
            Self::COLUMNS
        )))
        .bind(workspace_id)
        .bind(gallery_id)
        .fetch_all(self.pool)
        .await?)
    }

    /// Fetches one photo, scoped to the owning workspace and gallery.
    pub async fn get(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        id: PhotoId,
    ) -> Result<Option<Photo>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3",
            Self::COLUMNS
        )))
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(id)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Lists a gallery's ready photos in creation order.
    pub async fn list_ready(
        &self,
        location: PhotoLocation,
    ) -> Result<Vec<ReadyPhoto>, DatabaseError> {
        Ok(sqlx::query_as(
            "SELECT id, width, height FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND status = 'ready'
             ORDER BY created_at, id",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .fetch_all(self.pool)
        .await?)
    }

    /// Whether the gallery at `location` holds `id` and it is ready.
    pub async fn is_ready(
        &self,
        location: PhotoLocation,
        id: PhotoId,
    ) -> Result<bool, DatabaseError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3 AND status = 'ready')",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(id)
        .fetch_one(self.pool)
        .await?)
    }

    /// Finds the workspace and gallery owning a photo, regardless of its status.
    pub async fn locate(&self, id: PhotoId) -> Result<Option<PhotoLocation>, DatabaseError> {
        Ok(
            sqlx::query_as("SELECT workspace_id, gallery_id FROM photos WHERE id = $1")
                .bind(id)
                .fetch_optional(self.pool)
                .await?,
        )
    }

    /// Marks a waiting or processing photo as processing; repeatable after a worker crash.
    pub async fn mark_processing(&self, id: PhotoId) -> Result<bool, DatabaseError> {
        Self::transition_in(self.pool, id, "processing", None).await
    }

    /// Marks a waiting or processing photo as ready.
    pub async fn mark_ready(&self, id: PhotoId) -> Result<bool, DatabaseError> {
        Self::transition_in(self.pool, id, "ready", None).await
    }

    /// Same as [`Self::mark_ready`], within the caller's transaction.
    pub async fn mark_ready_in(
        transaction: &mut Transaction<'_, Postgres>,
        id: PhotoId,
    ) -> Result<bool, DatabaseError> {
        Self::transition_in(&mut **transaction, id, "ready", None).await
    }

    /// Whether the photo exists, within the caller's transaction.
    pub async fn exists_in(
        transaction: &mut Transaction<'_, Postgres>,
        id: PhotoId,
    ) -> Result<bool, DatabaseError> {
        Ok(
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM photos WHERE id = $1)")
                .bind(id)
                .fetch_one(&mut **transaction)
                .await?,
        )
    }

    /// Marks a waiting or processing photo as failed with a user-visible reason.
    pub async fn mark_failed(&self, id: PhotoId, reason: &str) -> Result<bool, DatabaseError> {
        Self::transition_in(self.pool, id, "failed", Some(reason)).await
    }

    /// Same as [`Self::mark_failed`], within the caller's transaction.
    pub async fn mark_failed_in(
        transaction: &mut Transaction<'_, Postgres>,
        id: PhotoId,
        reason: &str,
    ) -> Result<bool, DatabaseError> {
        Self::transition_in(&mut **transaction, id, "failed", Some(reason)).await
    }

    /// Moves an unfinished photo to `status`; returns whether a row changed.
    async fn transition_in(
        executor: impl PgExecutor<'_>,
        id: PhotoId,
        status: &str,
        failure_reason: Option<&str>,
    ) -> Result<bool, DatabaseError> {
        let result = sqlx::query(
            "UPDATE photos SET status = $2::photo_status, failure_reason = $3
             WHERE id = $1 AND status IN ('waiting', 'processing')",
        )
        .bind(id)
        .bind(status)
        .bind(failure_reason)
        .execute(executor)
        .await?;

        Ok(result.rows_affected() == 1)
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};

    use pp_core::notifications::PhotoId;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        photo_intake::PhotoIntake,
        tests::TestRepository,
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
    async fn decoding_text_status_preserves_valid_states_and_rejects_inconsistent_rows(
        pool: PgPool,
    ) {
        for (status, reason, expected) in [
            ("waiting", None, Some(PhotoStatus::Waiting)),
            ("processing", None, Some(PhotoStatus::Processing)),
            ("ready", None, Some(PhotoStatus::Ready)),
            (
                "failed",
                Some("decode failed"),
                Some(PhotoStatus::Failed("decode failed".to_owned())),
            ),
            ("failed", None, None),
            ("waiting", Some("unexpected"), None),
            ("processing", Some("unexpected"), None),
            ("ready", Some("unexpected"), None),
            ("unknown", None, None),
            ("unknown", Some("unexpected"), None),
        ] {
            let row = sqlx::query(
                "SELECT $1::uuid AS id, 'proof.jpg'::text AS original_name,
                 1024::bigint AS size, 20::integer AS width, 10::integer AS height,
                 $2::text AS status, $3::text AS failure_reason, now() AS created_at",
            )
            .bind(Uuid::new_v4())
            .bind(status)
            .bind(reason)
            .fetch_one(&pool)
            .await
            .unwrap();
            let decoded = Photo::from_row(&row);

            match expected {
                Some(expected) => assert_eq!(decoded.unwrap().status, expected),
                None => assert!(matches!(decoded, Err(sqlx::Error::Decode(_)))),
            }
        }
    }

    #[test]
    fn status_serialization_keeps_the_external_tag() {
        for (status, expected) in [
            (PhotoStatus::Waiting, serde_json::json!("waiting")),
            (PhotoStatus::Processing, serde_json::json!("processing")),
            (PhotoStatus::Ready, serde_json::json!("ready")),
            (
                PhotoStatus::Failed("decode failed".to_owned()),
                serde_json::json!({"failed": "decode failed"}),
            ),
        ] {
            assert_eq!(serde_json::to_value(&status).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<PhotoStatus>(expected).unwrap(),
                status
            );
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
                .try_accept(new_photo(workspace_id, gallery.id, id, original_name))
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

    async fn photo_in(
        database: &Database,
        location: PhotoLocation,
        (width, height): (u32, u32),
        status: PhotoStatus,
        created_at: i64,
    ) -> PhotoId {
        let id = PhotoId::from(Uuid::new_v4());
        let mut photo = new_photo(location.workspace_id, location.gallery_id, id, "proof.jpg");
        let tests = database.repository::<TestRepository>();

        photo.dimensions = PhotoDimensions {
            width: NonZeroU32::new(width).unwrap(),
            height: NonZeroU32::new(height).unwrap(),
        };
        database
            .repository::<PhotoIntake>()
            .try_accept(photo)
            .await
            .unwrap()
            .unwrap();
        tests.force_photo_status(id, &status).await.unwrap();
        tests
            .set_photo_created_at(id, OffsetDateTime::from_unix_timestamp(created_at).unwrap())
            .await
            .unwrap();

        id
    }

    async fn gallery_location(database: &Database, pool: &PgPool) -> PhotoLocation {
        let workspace_id = workspace(pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();

        PhotoLocation {
            workspace_id,
            gallery_id: gallery.id,
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn list_ready_returns_only_ready_photos_of_that_gallery_oldest_first(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let other = gallery_location(&database, &pool).await;
        let newer = photo_in(&database, location, (30, 40), PhotoStatus::Ready, 30).await;
        let older = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 10).await;

        for (status, created_at) in [
            (PhotoStatus::Waiting, 5),
            (PhotoStatus::Processing, 15),
            (PhotoStatus::Failed("broken".to_owned()), 25),
        ] {
            photo_in(&database, location, (20, 10), status, created_at).await;
        }

        photo_in(&database, other, (20, 10), PhotoStatus::Ready, 1).await;

        let listed = database
            .repository::<PhotoRepository>()
            .list_ready(location)
            .await
            .unwrap();
        let dimensions = |width, height| PhotoDimensions {
            width: NonZeroU32::new(width).unwrap(),
            height: NonZeroU32::new(height).unwrap(),
        };

        assert_eq!(
            listed,
            [
                ReadyPhoto {
                    id: older,
                    dimensions: dimensions(20, 10)
                },
                ReadyPhoto {
                    id: newer,
                    dimensions: dimensions(30, 40)
                },
            ]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn is_ready_is_false_for_unready_foreign_and_unknown_ids(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let other = gallery_location(&database, &pool).await;
        let ready = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let processing = photo_in(&database, location, (20, 10), PhotoStatus::Processing, 2).await;
        let foreign = photo_in(&database, other, (20, 10), PhotoStatus::Ready, 3).await;
        let photos = database.repository::<PhotoRepository>();

        assert!(photos.is_ready(location, ready).await.unwrap());

        for id in [processing, foreign, PhotoId::from(Uuid::new_v4())] {
            assert!(!photos.is_ready(location, id).await.unwrap());
        }
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
            .try_accept(new_photo(workspace_id, gallery.id, photo_id, "proof.jpg"))
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
                .try_accept(new_photo(
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

    async fn accepted_photo(
        database: &Database,
        pool: &PgPool,
    ) -> (WorkspaceId, GalleryId, PhotoId) {
        let workspace_id = workspace(pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photo_id = PhotoId::from(Uuid::new_v4());

        database
            .repository::<PhotoIntake>()
            .try_accept(new_photo(workspace_id, gallery.id, photo_id, "proof.jpg"))
            .await
            .unwrap()
            .unwrap();

        (workspace_id, gallery.id, photo_id)
    }

    async fn status(
        photos: &PhotoRepository<'_>,
        ids: (WorkspaceId, GalleryId, PhotoId),
    ) -> PhotoStatus {
        let (workspace_id, gallery_id, photo_id) = ids;

        photos
            .get(workspace_id, gallery_id, photo_id)
            .await
            .unwrap()
            .unwrap()
            .status
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn mark_ready_requires_a_waiting_or_processing_photo(pool: PgPool) {
        let database = Database::new(pool.clone());
        let ids = accepted_photo(&database, &pool).await;
        let photos = database.repository::<PhotoRepository>();

        assert!(photos.mark_ready(ids.2).await.unwrap());
        assert!(!photos.mark_ready(ids.2).await.unwrap());
        assert!(!photos.mark_failed(ids.2, "late failure").await.unwrap());
        assert!(!photos.mark_processing(ids.2).await.unwrap());
        assert_eq!(status(&photos, ids).await, PhotoStatus::Ready);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn mark_failed_stores_the_reason_and_listing_shows_it(pool: PgPool) {
        let database = Database::new(pool.clone());
        let ids = accepted_photo(&database, &pool).await;
        let photos = database.repository::<PhotoRepository>();

        assert!(photos.mark_processing(ids.2).await.unwrap());
        assert!(photos.mark_failed(ids.2, "decode failed").await.unwrap());
        assert!(!photos.mark_ready(ids.2).await.unwrap());

        let listed = photos.list(ids.0, ids.1).await.unwrap();

        assert_eq!(
            listed[0].status,
            PhotoStatus::Failed("decode failed".to_owned())
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn mark_failed_in_obeys_the_callers_transaction(pool: PgPool) {
        let database = Database::new(pool.clone());
        let ids = accepted_photo(&database, &pool).await;
        let photos = database.repository::<PhotoRepository>();
        let mut transaction = pool.begin().await.unwrap();

        assert!(
            PhotoRepository::mark_failed_in(&mut transaction, ids.2, PROCESSING_FAILED_REASON)
                .await
                .unwrap()
        );
        transaction.rollback().await.unwrap();

        assert_eq!(status(&photos, ids).await, PhotoStatus::Waiting);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn mark_processing_is_repeatable_after_a_crash(pool: PgPool) {
        let database = Database::new(pool.clone());
        let ids = accepted_photo(&database, &pool).await;
        let photos = database.repository::<PhotoRepository>();

        assert!(photos.mark_processing(ids.2).await.unwrap());
        assert!(photos.mark_processing(ids.2).await.unwrap());
        assert_eq!(status(&photos, ids).await, PhotoStatus::Processing);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn get_is_scoped_to_workspace_and_gallery(pool: PgPool) {
        let database = Database::new(pool.clone());
        let (workspace_id, gallery_id, photo_id) = accepted_photo(&database, &pool).await;
        let other_workspace = workspace(&pool).await;
        let other_gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photos = database.repository::<PhotoRepository>();

        assert!(
            photos
                .get(workspace_id, gallery_id, photo_id)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            photos
                .get(other_workspace, gallery_id, photo_id)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            photos
                .get(workspace_id, other_gallery.id, photo_id)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            photos.locate(photo_id).await.unwrap(),
            Some(PhotoLocation {
                workspace_id,
                gallery_id
            })
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn locate_returns_none_for_a_missing_photo(pool: PgPool) {
        let photos = Database::new(pool.clone());
        let photos = photos.repository::<PhotoRepository>();

        assert_eq!(
            photos.locate(PhotoId::from(Uuid::new_v4())).await.unwrap(),
            None
        );
    }
}
