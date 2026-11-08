//! Workspace-scoped photo metadata.
//!
//! A photo is visible only through its owning workspace and gallery. [`PhotoRepository`] stores
//! photo metadata and its processing status; [`crate::photo_intake::PhotoIntake`] coordinates the
//! insert with its processing job in one transaction.

use std::num::{NonZeroU32, NonZeroU64};

use derive_more::{Display, From};
use pp_core::types::{generate::Generate, timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{
    AssertSqlSafe, FromRow, PgExecutor, PgPool, Postgres, Row, Transaction, postgres::PgRow,
};
use uuid::Uuid;

use crate::{Repository, auth::WorkspaceId, error::DatabaseError, gallery::GalleryId};

/// Identifies a photo.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, Serialize, Deserialize,
)]
#[serde(transparent)]
#[derive(sqlx::Type)]
#[sqlx(transparent)]
pub struct PhotoId(Uuid);

impl Generate for PhotoId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

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

/// Which of a gallery's two independent photo sets a photo belongs to.
///
/// Every [`PhotoRepository`] query that reads a gallery's photos takes the kind it wants, so
/// a final is only ever returned to a caller that asked for one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "photo_kind", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum PhotoKind {
    /// A photo clients choose favorites from. Events from before kinds existed default to it.
    #[default]
    Proof,
    /// A finished photo the photographer prepares privately for delivery.
    Final,
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
    pub revision: i64,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub dimensions: PhotoDimensions,
}

/// A photo exposed by the workspace-scoped photo repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Photo {
    pub id: PhotoId,
    /// Clean proof derivatives are published and remain usable during marked processing/failure.
    pub clean_ready: bool,
    /// Changes only when a newly rendered asset is published as ready.
    pub revision: i64,
    pub original_name: String,
    pub size: NonZeroU64,
    #[serde(flatten)]
    pub dimensions: PhotoDimensions,
    pub status: PhotoStatus,
    /// Whether the client has picked it.
    pub favorite: bool,
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
            clean_ready: row.try_get("clean_ready")?,
            revision: row.try_get("revision")?,
            original_name: row.try_get("original_name")?,
            size,
            dimensions,
            status,
            favorite: row.try_get("favorite")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

/// Data recorded for an accepted original photo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPhoto {
    pub id: PhotoId,
    pub kind: PhotoKind,
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

/// Whether a caller wants the marked files of a proof rather than the clean ones.
///
/// Callers choose it; the repository only checks that those files are ready. Clean files are
/// ready as soon as they are published, even while marked copies are being regenerated; marked
/// ones only at the workspace's current watermark revision.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Watermarked(pub bool);

impl Watermarked {
    /// SQL condition over a `photos` row aliased `p`. Finals have a single set of files, so
    /// they are ready whenever their status is.
    const fn ready_condition(self) -> &'static str {
        if self.0 {
            "CASE WHEN p.kind = 'proof' THEN p.clean_ready AND
                p.watermark_revision = COALESCE(
                    (SELECT s.watermark_revision FROM workspace_settings s
                     WHERE s.workspace_id = p.workspace_id), 0)
                ELSE p.status = 'ready' END"
        } else {
            "CASE WHEN p.kind = 'proof' THEN p.clean_ready ELSE p.status = 'ready' END"
        }
    }
}

/// What [`PhotoRepository::set_favorite`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FavoriteChange {
    /// The photo is now, or already was, in the requested state.
    Applied,
    /// The gallery has no such ready photo.
    NotFound,
    /// The gallery is delivered, so its selection is final.
    Delivered,
}

/// Persistence for accepted photos. Queries that read a gallery's photos take the
/// [`PhotoKind`] to read, and never return a photo removed from a draft.
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
    const COLUMNS: &'static str = "id, clean_ready, revision, original_name, size, width, height, status::text AS status, failure_reason, favorite, created_at";

    /// Inserts only photo metadata in the caller's transaction.
    ///
    /// Locks the workspace before inserting, so processing-settings changes cannot miss a
    /// new upload and workspace/gallery foreign-key locks keep a consistent order.
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

        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR KEY SHARE")
            .bind(photo.workspace_id)
            .fetch_optional(&mut **transaction)
            .await?;

        let inserted: Option<Photo> = sqlx::query_as(
            "INSERT INTO photos
                 (id, workspace_id, gallery_id, original_name, size, width, height, kind)
             SELECT $1, $2, g.id, $4, $5, $6, $7, $8
             FROM galleries AS g
             WHERE g.id = $3 AND g.workspace_id = $2
             ON CONFLICT (id) DO NOTHING
             RETURNING id, clean_ready, revision, original_name, size, width, height,
                       status::text AS status, failure_reason, favorite, created_at",
        )
        .bind(photo.id)
        .bind(photo.workspace_id)
        .bind(photo.gallery_id)
        .bind(&photo.original_name)
        .bind(size)
        .bind(width)
        .bind(height)
        .bind(photo.kind)
        .fetch_optional(&mut **transaction)
        .await?;

        if let Some(inserted) = inserted {
            return Ok(PhotoInsertOutcome::Inserted(inserted));
        }

        let existing = sqlx::query(
            "SELECT id, clean_ready, revision, workspace_id, gallery_id, original_name, size, width, height, kind,
                    status::text AS status, failure_reason, favorite, created_at
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
            && row.try_get::<PhotoKind, _>("kind")? == photo.kind
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
    ///
    /// With a `variant`, only photos whose files for it are ready are returned.
    pub async fn list(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        kind: PhotoKind,
        variant: Option<Watermarked>,
    ) -> Result<Vec<Photo>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM photos AS p
             WHERE workspace_id = $1 AND gallery_id = $2 AND kind = $3 AND removed_at IS NULL
               AND {}
             ORDER BY created_at, id",
            Self::COLUMNS,
            variant.map_or("true", Watermarked::ready_condition)
        )))
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(kind)
        .fetch_all(self.pool)
        .await?)
    }

    /// Fetches one photo, scoped to the owning workspace and gallery.
    pub async fn get(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        id: PhotoId,
        kind: PhotoKind,
    ) -> Result<Option<Photo>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3
               AND kind = $4 AND removed_at IS NULL",
            Self::COLUMNS
        )))
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(id)
        .bind(kind)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Lists a gallery's photos whose files for `variant` are ready, in creation order.
    ///
    /// Clean files stay ready while marked copies are regenerated; marked files are ready only
    /// at the workspace's current watermark revision.
    pub async fn list_ready(
        &self,
        location: PhotoLocation,
        kind: PhotoKind,
        variant: Watermarked,
    ) -> Result<Vec<ReadyPhoto>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT id, revision, width, height FROM photos AS p
             WHERE workspace_id = $1 AND gallery_id = $2 AND kind = $3 AND removed_at IS NULL
               AND {}
             ORDER BY created_at, id",
            variant.ready_condition()
        )))
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(kind)
        .fetch_all(self.pool)
        .await?)
    }

    /// Whether the gallery at `location` holds `id` with its files for `variant` ready.
    pub async fn is_ready(
        &self,
        location: PhotoLocation,
        id: PhotoId,
        kind: PhotoKind,
        variant: Watermarked,
    ) -> Result<bool, DatabaseError> {
        Ok(sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT EXISTS(SELECT 1 FROM photos AS p
             WHERE workspace_id = $1 AND gallery_id = $2 AND id = $3
               AND kind = $4 AND removed_at IS NULL AND {})",
            variant.ready_condition()
        )))
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(id)
        .bind(kind)
        .fetch_one(self.pool)
        .await?)
    }

    /// Adds `photo` to the gallery's favorites, or removes it when `favorite` is false.
    ///
    /// One conditional `UPDATE` decides everything, so repeated and concurrent calls converge. The
    /// photo must be one of the gallery at `location` with its files for `variant` ready; a
    /// delivered gallery's selection no longer changes.
    pub async fn set_favorite(
        &self,
        location: PhotoLocation,
        id: PhotoId,
        favorite: bool,
        kind: PhotoKind,
        variant: Watermarked,
    ) -> Result<FavoriteChange, DatabaseError> {
        let delivered: Option<bool> = sqlx::query_scalar(AssertSqlSafe(format!(
            "WITH target AS (
                 SELECT p.id, g.delivered_at IS NOT NULL AS delivered
                 FROM photos AS p JOIN galleries AS g ON g.id = p.gallery_id
                 WHERE p.id = $3 AND p.workspace_id = $1 AND p.gallery_id = $2
                   AND p.kind = $5 AND p.removed_at IS NULL AND {}
             ),
             changed AS (
                 UPDATE photos SET favorite = $4
                 WHERE id IN (SELECT id FROM target WHERE NOT delivered)
                   AND favorite IS DISTINCT FROM $4
             )
             SELECT delivered FROM target",
            variant.ready_condition()
        )))
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(id)
        .bind(favorite)
        .bind(kind)
        .fetch_optional(self.pool)
        .await?;

        Ok(match delivered {
            None => FavoriteChange::NotFound,
            Some(true) => FavoriteChange::Delivered,
            Some(false) => FavoriteChange::Applied,
        })
    }

    /// Lists the ids of the gallery's favorites in the same order as its photos.
    pub async fn favorites(
        &self,
        location: PhotoLocation,
        kind: PhotoKind,
    ) -> Result<Vec<PhotoId>, DatabaseError> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM photos
             WHERE workspace_id = $1 AND gallery_id = $2 AND favorite AND kind = $3
             ORDER BY created_at, id",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(kind)
        .fetch_all(self.pool)
        .await?)
    }

    /// Finds the workspace and gallery owning a photo of either kind, regardless of its status.
    /// Workers and cleanup use it; nothing a client reaches may.
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
            "UPDATE photos SET status = $2::photo_status, failure_reason = $3,
                 clean_ready = clean_ready OR $2 = 'ready',
                 watermark_revision = CASE WHEN $2 = 'ready' AND kind = 'proof'
                     THEN COALESCE((SELECT watermark_revision FROM workspace_settings
                                    WHERE workspace_id = photos.workspace_id), 0)
                     ELSE watermark_revision END,
                 revision = revision + CASE WHEN $2 = 'ready' THEN 1 ELSE 0 END
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
    use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use super::*;
    use crate::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        jobs::{DEFAULT_CLEANUP_BATCH, JobOptions},
        listener::Notification,
        photo_events::{PhotoChange, PhotoEvent},
        photo_intake::PhotoIntake,
        tests::TestRepository,
        workspace_deletion::WorkspaceDeletion,
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
            kind: PhotoKind::Proof,
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
    async fn regenerated_photo_gets_a_new_asset_revision(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace_id = workspace(&pool).await;
        let gallery = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap();
        let photo_id = PhotoId::from(Uuid::new_v4());
        let photos = database.repository::<PhotoRepository>();
        let mut transaction = pool.begin().await.unwrap();
        photos
            .insert_in(
                &mut transaction,
                &new_photo(workspace_id, gallery.id, photo_id, "a.jpg"),
            )
            .await
            .unwrap();
        transaction.commit().await.unwrap();
        photos.mark_ready(photo_id).await.unwrap();
        let first = photos
            .get(workspace_id, gallery.id, photo_id, PhotoKind::Proof)
            .await
            .unwrap()
            .unwrap();
        sqlx::query("UPDATE photos SET status = 'waiting' WHERE id = $1")
            .bind(photo_id)
            .execute(&pool)
            .await
            .unwrap();
        photos.mark_ready(photo_id).await.unwrap();
        let second = photos
            .get(workspace_id, gallery.id, photo_id, PhotoKind::Proof)
            .await
            .unwrap()
            .unwrap();

        assert_ne!(
            serde_json::to_value(first).unwrap()["revision"],
            serde_json::to_value(second).unwrap()["revision"]
        );
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
                "SELECT $1::uuid AS id, false AS clean_ready, 0::bigint AS revision, 'proof.jpg'::text AS original_name,
                 1024::bigint AS size, 20::integer AS width, 10::integer AS height,
                 $2::text AS status, $3::text AS failure_reason, false AS favorite, now() AS created_at",
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

        let listed = photos
            .list(workspace_id, gallery.id, PhotoKind::Proof, None)
            .await
            .unwrap();

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
            .list_ready(location, PhotoKind::Proof, Watermarked(false))
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
                    revision: 0,
                    dimensions: dimensions(20, 10)
                },
                ReadyPhoto {
                    id: newer,
                    revision: 0,
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

        assert!(
            photos
                .is_ready(location, ready, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap()
        );

        for id in [processing, foreign, PhotoId::from(Uuid::new_v4())] {
            assert!(
                !photos
                    .is_ready(location, id, PhotoKind::Proof, Watermarked(false))
                    .await
                    .unwrap()
            );
        }
    }

    async fn deliver(pool: &PgPool, location: PhotoLocation) {
        sqlx::query("UPDATE galleries SET delivered_at = now() WHERE id = $1")
            .bind(location.gallery_id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn favorites_are_set_and_cleared_idempotently_and_listed_oldest_first(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let older = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let newer = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 2).await;
        let photos = database.repository::<PhotoRepository>();

        assert!(
            photos
                .favorites(location, PhotoKind::Proof)
                .await
                .unwrap()
                .is_empty()
        );

        for id in [newer, older, newer] {
            assert_eq!(
                photos
                    .set_favorite(location, id, true, PhotoKind::Proof, Watermarked(false))
                    .await
                    .unwrap(),
                FavoriteChange::Applied
            );
        }

        assert_eq!(
            photos.favorites(location, PhotoKind::Proof).await.unwrap(),
            [older, newer]
        );

        for _ in 0..2 {
            assert_eq!(
                photos
                    .set_favorite(location, older, false, PhotoKind::Proof, Watermarked(false))
                    .await
                    .unwrap(),
                FavoriteChange::Applied
            );
        }

        assert_eq!(
            photos.favorites(location, PhotoKind::Proof).await.unwrap(),
            [newer]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn favorites_reject_unready_foreign_and_unknown_photos(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let other = gallery_location(&database, &pool).await;
        let processing = photo_in(&database, location, (20, 10), PhotoStatus::Processing, 1).await;
        let foreign = photo_in(&database, other, (20, 10), PhotoStatus::Ready, 2).await;
        let photos = database.repository::<PhotoRepository>();

        for id in [processing, foreign, PhotoId::from(Uuid::new_v4())] {
            assert_eq!(
                photos
                    .set_favorite(location, id, true, PhotoKind::Proof, Watermarked(false))
                    .await
                    .unwrap(),
                FavoriteChange::NotFound
            );
        }

        assert!(
            photos
                .favorites(location, PhotoKind::Proof)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            photos
                .favorites(other, PhotoKind::Proof)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_delivered_gallery_keeps_its_selection(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let chosen = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let spare = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 2).await;
        let photos = database.repository::<PhotoRepository>();

        photos
            .set_favorite(location, chosen, true, PhotoKind::Proof, Watermarked(false))
            .await
            .unwrap();
        deliver(&pool, location).await;

        for (id, favorite) in [(chosen, false), (spare, true)] {
            assert_eq!(
                photos
                    .set_favorite(location, id, favorite, PhotoKind::Proof, Watermarked(false))
                    .await
                    .unwrap(),
                FavoriteChange::Delivered
            );
        }

        assert_eq!(
            photos.favorites(location, PhotoKind::Proof).await.unwrap(),
            [chosen]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn concurrent_toggles_converge(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let id = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let photos = database.repository::<PhotoRepository>();
        let (first, second, third) = tokio::join!(
            photos.set_favorite(location, id, true, PhotoKind::Proof, Watermarked(false)),
            photos.set_favorite(location, id, true, PhotoKind::Proof, Watermarked(false)),
            photos.set_favorite(location, id, true, PhotoKind::Proof, Watermarked(false)),
        );

        for result in [first, second, third] {
            assert_eq!(result.unwrap(), FavoriteChange::Applied);
        }

        assert_eq!(
            photos.favorites(location, PhotoKind::Proof).await.unwrap(),
            [id]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_favorite_removes_only_it(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let doomed = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let kept = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 2).await;
        let photos = database.repository::<PhotoRepository>();

        for id in [doomed, kept] {
            photos
                .set_favorite(location, id, true, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap();
        }

        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &[doomed],
            )
            .await
            .unwrap();

        assert_eq!(
            photos.favorites(location, PhotoKind::Proof).await.unwrap(),
            [kept]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn the_workspace_lists_its_favorite_photos_with_the_flag(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let chosen = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let spare = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 2).await;
        let photos = database.repository::<PhotoRepository>();

        photos
            .set_favorite(location, chosen, true, PhotoKind::Proof, Watermarked(false))
            .await
            .unwrap();

        let listed = photos
            .list(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                None,
            )
            .await
            .unwrap();

        assert_eq!(
            listed
                .iter()
                .map(|photo| (photo.id, photo.favorite))
                .collect::<Vec<_>>(),
            [(chosen, true), (spare, false)]
        );
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

        assert!(
            photos
                .list(other, gallery.id, PhotoKind::Proof, None)
                .await
                .unwrap()
                .is_empty()
        );
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
            .get(workspace_id, gallery_id, photo_id, PhotoKind::Proof)
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

        let listed = photos
            .list(ids.0, ids.1, PhotoKind::Proof, None)
            .await
            .unwrap();

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
                .get(workspace_id, gallery_id, photo_id, PhotoKind::Proof)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            photos
                .get(other_workspace, gallery_id, photo_id, PhotoKind::Proof)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            photos
                .get(workspace_id, other_gallery.id, photo_id, PhotoKind::Proof)
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

    /// Gives `id` one extra job in each state, besides the one its intake queued.
    async fn jobs_in_every_state(pool: &PgPool, id: PhotoId) {
        let payload = serde_json::json!({"kind": "process_photo", "payload": {"photo_id": id}});

        sqlx::query(
            "INSERT INTO jobs(id, payload, state, max_attempts, locked_until, last_error)
             VALUES (gen_random_uuid(), $1, 'running', 3, now() + interval '1 minute', NULL),
                    (gen_random_uuid(), $1, 'failed', 3, NULL, 'gave up')",
        )
        .bind(payload)
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_removes_the_photo_and_all_its_jobs_whatever_its_status(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let tests = database.repository::<TestRepository>();
        let photos = database.repository::<PhotoRepository>();
        let kept_photo = photo_in(&database, location, (20, 10), PhotoStatus::Waiting, 1).await;

        for status in [
            PhotoStatus::Waiting,
            PhotoStatus::Processing,
            PhotoStatus::Ready,
            PhotoStatus::Failed("broken".to_owned()),
        ] {
            let id = photo_in(&database, location, (20, 10), status, 2).await;

            jobs_in_every_state(&pool, id).await;

            assert_eq!(
                database
                    .repository::<WorkspaceDeletion>()
                    .delete_photos(
                        location.workspace_id,
                        location.gallery_id,
                        PhotoKind::Proof,
                        &[id]
                    )
                    .await
                    .unwrap(),
                vec![id]
            );
            assert_eq!(
                photos
                    .get(
                        location.workspace_id,
                        location.gallery_id,
                        id,
                        PhotoKind::Proof
                    )
                    .await
                    .unwrap(),
                None
            );
            assert!(tests.photo_jobs(id).await.unwrap().is_empty());
        }

        assert_eq!(tests.photo_jobs(kept_photo).await.unwrap().len(), 1);
        assert_eq!(tests.count_photos().await.unwrap(), 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_queues_cleanup_for_exactly_the_removed_photos(pool: PgPool) {
        let database = Database::new(pool.clone());
        let tests = database.repository::<TestRepository>();
        let location = gallery_location(&database, &pool).await;
        let other = gallery_location(&database, &pool).await;
        let first = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let kept = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 2).await;
        let foreign = photo_in(&database, other, (20, 10), PhotoStatus::Ready, 3).await;

        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &[first, foreign],
            )
            .await
            .unwrap();
        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &[first],
            )
            .await
            .unwrap();
        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &[kept],
            )
            .await
            .unwrap();

        let cleanups = tests.jobs_of_kind("cleanup_photos").await.unwrap();
        let named: Vec<_> = cleanups
            .iter()
            .map(|job| job.payload["payload"]["photo_ids"].clone())
            .collect();

        assert_eq!(
            named,
            [
                serde_json::json!([first.to_string()]),
                serde_json::json!([kept.to_string()])
            ]
        );
        assert_eq!(
            cleanups[0].payload["payload"]["gallery_id"],
            location.gallery_id.0.to_string()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_honors_the_configured_cleanup_batch(pool: PgPool) {
        let database = Database::new(pool.clone());
        let deletion = database.repository_with::<WorkspaceDeletion>(JobOptions {
            cleanup_batch: NonZeroUsize::new(2).unwrap(),
            ..JobOptions::default()
        });
        let location = gallery_location(&database, &pool).await;
        let mut ids = Vec::new();

        for _ in 0..3 {
            ids.push(photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await);
        }

        deletion
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &ids,
            )
            .await
            .unwrap();

        assert_eq!(
            database
                .repository::<TestRepository>()
                .jobs_of_kind("cleanup_photos")
                .await
                .unwrap()
                .len(),
            2
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_splits_large_cleanups_and_keeps_earlier_cleanup_jobs(pool: PgPool) {
        let database = Database::new(pool.clone());
        let tests = database.repository::<TestRepository>();
        let location = gallery_location(&database, &pool).await;
        let mut ids = Vec::new();

        for _ in 0..=DEFAULT_CLEANUP_BATCH.get() {
            ids.push(PhotoId::from(Uuid::new_v4()));
        }

        sqlx::query(
            "INSERT INTO photos(id, workspace_id, gallery_id, original_name, size, width, height, status)
             SELECT unnest($3::uuid[]), $1, $2, 'p.jpg', 1, 1, 1, 'ready'",
        )
        .bind(location.workspace_id)
        .bind(location.gallery_id)
        .bind(&ids)
        .execute(&pool)
        .await
        .unwrap();

        let deleted = database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &ids,
            )
            .await
            .unwrap();

        assert_eq!(deleted.len(), ids.len());
        assert_eq!(tests.jobs_of_kind("cleanup_photos").await.unwrap().len(), 2);

        let more = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;

        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &[more],
            )
            .await
            .unwrap();

        assert_eq!(tests.jobs_of_kind("cleanup_photos").await.unwrap().len(), 3);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_waits_for_a_worker_finishing_the_photo(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let id = photo_in(&database, location, (20, 10), PhotoStatus::Processing, 1).await;
        let mut worker_transaction = pool.begin().await.unwrap();

        // Completing a job locks its row first, then the photo's.
        sqlx::query("DELETE FROM jobs WHERE payload->'payload'->>'photo_id' = $1")
            .bind(id.to_string())
            .execute(&mut *worker_transaction)
            .await
            .unwrap();

        let delete_task = tokio::spawn({
            let database = database.clone();

            async move {
                database
                    .repository::<WorkspaceDeletion>()
                    .delete_photos(
                        location.workspace_id,
                        location.gallery_id,
                        PhotoKind::Proof,
                        &[id],
                    )
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
            .execute(&mut *worker_transaction)
            .await
            .unwrap();
        worker_transaction.commit().await.unwrap();

        assert_eq!(delete_task.await.unwrap().unwrap(), vec![id]);
        assert_eq!(
            database
                .repository::<TestRepository>()
                .count_photos()
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_is_scoped_to_workspace_and_gallery(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let other_location = gallery_location(&database, &pool).await;
        let photos = database.repository::<PhotoRepository>();
        let id = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;

        for (workspace_id, gallery_id, photo_id) in [
            (other_location.workspace_id, location.gallery_id, id),
            (location.workspace_id, other_location.gallery_id, id),
            (
                location.workspace_id,
                location.gallery_id,
                PhotoId::from(Uuid::new_v4()),
            ),
        ] {
            assert!(
                database
                    .repository::<WorkspaceDeletion>()
                    .delete_photos(workspace_id, gallery_id, PhotoKind::Proof, &[photo_id])
                    .await
                    .unwrap()
                    .is_empty()
            );
        }

        assert!(
            photos
                .get(
                    location.workspace_id,
                    location.gallery_id,
                    id,
                    PhotoKind::Proof
                )
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            database
                .repository::<TestRepository>()
                .photo_jobs(id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_removes_several_photos_and_ignores_other_workspaces_and_unknown_ids(
        pool: PgPool,
    ) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let other_workspace = gallery_location(&database, &pool).await;
        let tests = database.repository::<TestRepository>();
        let first = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let second = photo_in(&database, location, (20, 10), PhotoStatus::Processing, 2).await;
        let kept = photo_in(&database, location, (20, 10), PhotoStatus::Waiting, 3).await;
        let foreign = photo_in(&database, other_workspace, (20, 10), PhotoStatus::Ready, 4).await;

        assert_ne!(other_workspace.workspace_id, location.workspace_id);

        jobs_in_every_state(&pool, first).await;

        let mut deleted = database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &[first, second, foreign, PhotoId::from(Uuid::new_v4()), first],
            )
            .await
            .unwrap();
        let mut expected = vec![first, second];

        deleted.sort();
        expected.sort();

        assert_eq!(deleted, expected);

        for id in [first, second] {
            assert!(tests.photo_jobs(id).await.unwrap().is_empty());
        }

        for id in [kept, foreign] {
            assert_eq!(tests.photo_jobs(id).await.unwrap().len(), 1);
        }

        assert_eq!(tests.count_photos().await.unwrap(), 2);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_announces_the_deletion(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let id = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let mut listener = database.listen::<PhotoEvent>().await.unwrap();

        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                &[id],
            )
            .await
            .unwrap();

        let Notification::Event(event) = listener.next().await.unwrap() else {
            panic!("expected a photo event");
        };

        assert_eq!(event.photo_id, id);
        assert_eq!(event.status, PhotoChange::Deleted);
    }

    async fn final_in(database: &Database, location: PhotoLocation) -> PhotoId {
        let id = PhotoId::from(Uuid::new_v4());
        let mut photo = new_photo(location.workspace_id, location.gallery_id, id, "final.jpg");

        photo.kind = PhotoKind::Final;
        database
            .repository::<PhotoIntake>()
            .try_accept(photo)
            .await
            .unwrap()
            .unwrap();
        database
            .repository::<TestRepository>()
            .force_photo_status(id, &PhotoStatus::Ready)
            .await
            .unwrap();

        id
    }

    async fn publish(pool: &PgPool, id: PhotoId) {
        sqlx::query("UPDATE photos SET published_at = now() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn proofs_and_finals_are_listed_and_fetched_separately(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let proof = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let finished = final_in(&database, location).await;
        let photos = database.repository::<PhotoRepository>();
        let (workspace_id, gallery_id) = (location.workspace_id, location.gallery_id);

        let listed: Vec<_> = photos
            .list(workspace_id, gallery_id, PhotoKind::Proof, None)
            .await
            .unwrap();
        let listed_finals = photos
            .list(workspace_id, gallery_id, PhotoKind::Final, None)
            .await
            .unwrap();

        assert_eq!(listed.iter().map(|p| p.id).collect::<Vec<_>>(), vec![proof]);
        assert_eq!(
            listed_finals.iter().map(|p| p.id).collect::<Vec<_>>(),
            vec![finished]
        );
        assert!(
            photos
                .get(workspace_id, gallery_id, finished, PhotoKind::Proof)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            photos
                .get(workspace_id, gallery_id, proof, PhotoKind::Final)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            photos
                .list_ready(location, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn removing_an_unpublished_final_deletes_it_and_queues_its_cleanup(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let finished = final_in(&database, location).await;

        let removed = database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Final,
                &[finished],
            )
            .await
            .unwrap();

        assert_eq!(removed, vec![finished]);
        assert_eq!(
            database
                .repository::<TestRepository>()
                .count_photos()
                .await
                .unwrap(),
            0
        );
        assert_eq!(
            database
                .repository::<TestRepository>()
                .jobs_of_kind("cleanup_photos")
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn removing_a_published_final_keeps_row_and_files_but_hides_it(pool: PgPool) {
        let database = Database::new(pool.clone());
        let tests = database.repository::<TestRepository>();
        let location = gallery_location(&database, &pool).await;
        let finished = final_in(&database, location).await;
        let deletion = database.repository::<WorkspaceDeletion>();
        let photos = database.repository::<PhotoRepository>();

        publish(&pool, finished).await;

        let first = deletion
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Final,
                &[finished],
            )
            .await
            .unwrap();
        let second = deletion
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Final,
                &[finished],
            )
            .await
            .unwrap();

        assert_eq!(first, vec![finished]);
        assert!(second.is_empty());
        assert_eq!(tests.count_photos().await.unwrap(), 1);
        assert!(
            tests
                .jobs_of_kind("cleanup_photos")
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            photos
                .list(
                    location.workspace_id,
                    location.gallery_id,
                    PhotoKind::Final,
                    None
                )
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            photos
                .get(
                    location.workspace_id,
                    location.gallery_id,
                    finished,
                    PhotoKind::Final
                )
                .await
                .unwrap()
                .is_none()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_proof_delete_cannot_remove_a_final_and_vice_versa(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let proof = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let finished = final_in(&database, location).await;
        let deletion = database.repository::<WorkspaceDeletion>();

        assert!(
            deletion
                .delete_photos(
                    location.workspace_id,
                    location.gallery_id,
                    PhotoKind::Proof,
                    &[finished]
                )
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            deletion
                .delete_photos(
                    location.workspace_id,
                    location.gallery_id,
                    PhotoKind::Final,
                    &[proof]
                )
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            database
                .repository::<TestRepository>()
                .count_photos()
                .await
                .unwrap(),
            2
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn draft_columns_are_only_valid_on_finals(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let proof = photo_in(&database, location, (20, 10), PhotoStatus::Ready, 1).await;
        let finished = final_in(&database, location).await;

        let published_proof = sqlx::query("UPDATE photos SET published_at = now() WHERE id = $1")
            .bind(proof)
            .execute(&pool)
            .await;
        let removed_unpublished = sqlx::query("UPDATE photos SET removed_at = now() WHERE id = $1")
            .bind(finished)
            .execute(&pool)
            .await;

        assert!(published_proof.is_err());
        assert!(removed_unpublished.is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn events_name_the_kind_and_report_soft_removal_as_deleted(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let mut listener = database.listen::<PhotoEvent>().await.unwrap();
        let finished = final_in(&database, location).await;

        publish(&pool, finished).await;
        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Final,
                &[finished],
            )
            .await
            .unwrap();

        let mut seen = Vec::new();

        while let Ok(Ok(Notification::Event(event))) =
            tokio::time::timeout(std::time::Duration::from_millis(500), listener.next()).await
        {
            seen.push((event.kind, event.status));
        }

        assert_eq!(
            seen.first(),
            Some(&(PhotoKind::Final, PhotoChange::Waiting))
        );
        assert_eq!(seen.last(), Some(&(PhotoKind::Final, PhotoChange::Deleted)));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn retry_ignores_other_kinds_and_removed_finals(pool: PgPool) {
        let database = Database::new(pool.clone());
        let location = gallery_location(&database, &pool).await;
        let finished = final_in(&database, location).await;
        let retry =
            database.repository_with::<crate::photo_retry::PhotoRetry>(JobOptions::default());

        database
            .repository::<TestRepository>()
            .force_photo_status(finished, &PhotoStatus::Failed("x".to_owned()))
            .await
            .unwrap();

        let as_proof = retry
            .retry_failed(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Proof,
                Some(finished),
            )
            .await
            .unwrap();
        let as_final = retry
            .retry_failed(
                location.workspace_id,
                location.gallery_id,
                PhotoKind::Final,
                Some(finished),
            )
            .await
            .unwrap();

        assert_eq!(as_proof, None);
        assert_eq!(as_final, Some(1));
    }
}
