//! Galleries owned by a workspace.
//!
//! Every query is scoped by `workspace_id`, so a gallery of another workspace is
//! indistinguishable from a missing one. Settings are edited through a [`GalleryChanges`]
//! changeset: a missing field keeps the stored value and, for optional settings, an explicit
//! `null` clears it. A gallery has no title until one is set; the default text belongs to the
//! front end, which knows the viewer's language. Value limits live in the database as CHECK constraints, which are the
//! single authority; violations surface as [`GalleryError`] variants.

use std::{num::NonZeroUsize, time::Duration};

use derive_more::Display;
use pp_core::types::{generate::Generate, timestamp::Timestamp};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sqlx::{AssertSqlSafe, FromRow, PgExecutor, PgPool, Row, Type, postgres::PgRow};
use thiserror::Error;
use uuid::Uuid;

pub use self::share::{ShareToken, SharedAccess, SharedGallery};
use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    photo::{PhotoId, PhotoLocation},
};

mod share;

/// Postgres SQLSTATE for a string containing a NUL byte.
const INVALID_BYTE_SEQUENCE: &str = "22021";

/// The CHECK constraint bounding the title length; see the migration.
const TITLE_LENGTH_CONSTRAINT: &str = "galleries_title_length";

/// A setting that failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum GallerySetting {
    #[display("title")]
    Title,
    #[display("included_photos")]
    IncludedPhotos,
}

#[derive(Debug, Error)]
pub enum GalleryError {
    #[error("{0} exceeds its length limit")]
    LengthOverflow(GallerySetting),
    #[error("{0} contains characters the database cannot store")]
    InvalidCharacters(GallerySetting),
    #[error("database error")]
    Database(#[from] DatabaseError),
}

impl GalleryError {
    /// The setting that caused a validation failure, if it was one.
    pub fn setting(&self) -> Option<GallerySetting> {
        match self {
            Self::LengthOverflow(setting) | Self::InvalidCharacters(setting) => Some(*setting),
            Self::Database(_) => None,
        }
    }

    /// Maps a validation failure to the setting that caused it; anything else stays a database
    /// error. Private so `sqlx::Error` stays out of this crate's public API.
    fn classify(error: sqlx::Error) -> Self {
        let violation = error.as_database_error().and_then(|db| {
            match (db.constraint(), db.code().as_deref()) {
                (Some(TITLE_LENGTH_CONSTRAINT), _) => {
                    Some(Self::LengthOverflow(GallerySetting::Title))
                }
                (_, Some(INVALID_BYTE_SEQUENCE)) => {
                    Some(Self::InvalidCharacters(GallerySetting::Title))
                }
                _ => None,
            }
        });

        violation.unwrap_or_else(|| Self::Database(DatabaseError::from(error)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct GalleryId(pub Uuid);

impl Generate for GalleryId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Default arrangement of photos for viewers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "gallery_layout", rename_all = "lowercase")]
pub enum GalleryLayout {
    #[default]
    Grid,
    Masonry,
}

/// The result of [`GalleryRepository::deliver`].
#[derive(Debug)]
pub enum Delivery {
    Delivered(Gallery),
    /// The gallery is not shared yet, or is archived.
    NotDeliverable,
    NotFound,
}

/// Where a gallery stands. Exactly one of `Draft`, `Shared` and `Delivered` always applies;
/// `Archived` and `Expired` are flags that can come on top of any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GalleryState {
    Draft,
    Shared,
    Delivered,
    Archived,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gallery {
    pub id: GalleryId,
    /// `None` means the gallery has no title yet.
    pub title: Option<String>,
    pub layout: GalleryLayout,
    /// `None` means the gallery never expires.
    pub expires_at: Option<Timestamp>,
    /// `None` means selection is unlimited.
    pub included_photos: Option<NonZeroUsize>,
    pub watermark_enabled: bool,
    /// `None` means the gallery has no hero photo.
    pub hero_id: Option<PhotoId>,
    /// `None` means the gallery has not been shared.
    pub share_token: Option<ShareToken>,
    /// When a client last opened the shared gallery, to within the throttle delay; `None` means
    /// not yet.
    pub last_opened_at: Option<Timestamp>,
    /// The progress state first, then `Archived` and `Expired` when they apply.
    pub states: Vec<GalleryState>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl Gallery {
    /// The photo allowance of a gallery row, where `None` means selection is unlimited.
    pub(crate) fn included_photos(row: &PgRow) -> Result<Option<NonZeroUsize>, sqlx::Error> {
        row.try_get::<Option<i32>, _>("included_photos")?
            .map(|count| {
                usize::try_from(count)
                    .ok()
                    .and_then(NonZeroUsize::new)
                    .ok_or_else(|| sqlx::Error::Decode("included_photos is not positive".into()))
            })
            .transpose()
    }
}

impl FromRow<'_, PgRow> for Gallery {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let included_photos = Self::included_photos(row)?;

        let progress = if row
            .try_get::<Option<Timestamp>, _>("delivered_at")?
            .is_some()
        {
            GalleryState::Delivered
        } else if row.try_get::<Option<Timestamp>, _>("shared_at")?.is_some() {
            GalleryState::Shared
        } else {
            GalleryState::Draft
        };
        let mut states = vec![progress];

        if row
            .try_get::<Option<Timestamp>, _>("archived_at")?
            .is_some()
        {
            states.push(GalleryState::Archived);
        }

        if row.try_get::<bool, _>("expired")? {
            states.push(GalleryState::Expired);
        }

        Ok(Self {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            layout: row.try_get("layout")?,
            expires_at: row.try_get("expires_at")?,
            included_photos,
            watermark_enabled: row.try_get("watermark_enabled")?,
            hero_id: row.try_get("hero_id")?,
            share_token: row.try_get("share_token")?,
            last_opened_at: row.try_get("last_opened_at")?,
            states,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

/// A gallery as listed on the dashboard, with totals computed from its photos at query time.
///
/// Only ready photos count, as they are the ones a client can see and select.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GalleryOverview {
    #[serde(flatten)]
    pub gallery: Gallery,
    pub photo_count: usize,
    /// How many of the ready photos are favorites.
    pub favorite_count: usize,
    /// How many ready finished photos the gallery holds.
    pub final_count: usize,
    /// How many of them the client has downloaded.
    pub downloaded_count: usize,
}

impl FromRow<'_, PgRow> for GalleryOverview {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let count = |column: &str| {
            usize::try_from(row.try_get::<i64, _>(column)?)
                .map_err(|error| sqlx::Error::Decode(error.into()))
        };

        Ok(Self {
            gallery: Gallery::from_row(row)?,
            photo_count: count("photo_count")?,
            favorite_count: count("favorite_count")?,
            final_count: count("final_count")?,
            downloaded_count: count("downloaded_count")?,
        })
    }
}

/// The settings to change; see the module docs for the keep and clear semantics.
///
/// For the optional settings the outer `Option` says whether the field was supplied and the
/// inner one is the new value, so `Some(None)` clears it. Non-optional settings reject `null`
/// and unknown fields are refused, so a typo never silently does nothing.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleryChanges {
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub title: Option<Option<String>>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub layout: Option<GalleryLayout>,
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<Option<Timestamp>>,
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub included_photos: Option<Option<NonZeroUsize>>,
    #[serde(
        default,
        deserialize_with = "deserialize_present",
        skip_serializing_if = "Option::is_none"
    )]
    pub watermark_enabled: Option<bool>,
    /// Must be a ready photo of the gallery; anything else leaves the gallery unchanged.
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub hero_id: Option<Option<PhotoId>>,
}

impl GalleryChanges {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Deserializes a supplied value as `Some` and rejects `null`.
///
/// Plain `Option<T>` fields treat `null` like a missing field, so `{"layout": null}` would
/// silently change nothing; settings that cannot be cleared fail instead.
fn deserialize_present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    // Going through `Option` makes `null` a data error for every `T`; deserializing `T`
    // directly makes some types (enums) report it as a syntax error instead.
    Option::<T>::deserialize(deserializer)?
        .map(Some)
        .ok_or_else(|| D::Error::custom("null is not allowed"))
}

/// Ordering of [`GalleryRepository::list`]; ties always break on the gallery id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GallerySort {
    /// Most recently edited first.
    #[default]
    Updated,
    /// Most recently created first.
    Created,
    /// Least recently created first.
    Oldest,
    /// Alphabetical, ignoring case.
    Title,
}

impl GallerySort {
    fn order_by(self) -> &'static str {
        match self {
            Self::Updated => "updated_at DESC, id",
            Self::Created => "created_at DESC, id",
            Self::Oldest => "created_at, id",
            Self::Title => "lower(title) NULLS LAST, id",
        }
    }
}

impl GalleryState {
    /// The `WHERE` condition matching galleries in this state.
    fn condition(self) -> &'static str {
        match self {
            Self::Draft => "(shared_at IS NULL AND delivered_at IS NULL)",
            Self::Shared => "(shared_at IS NOT NULL AND delivered_at IS NULL)",
            Self::Delivered => "(delivered_at IS NOT NULL)",
            Self::Archived => "(archived_at IS NOT NULL)",
            Self::Expired => "COALESCE(expires_at <= now(), false)",
        }
    }
}

/// Data access for galleries; see the module docs.
#[derive(Clone, Copy)]
pub struct GalleryRepository<'db> {
    pool: &'db PgPool,
}

impl<'db> Repository<'db> for GalleryRepository<'db> {
    type Options = ();

    fn open(pool: &'db PgPool, (): Self::Options) -> Self {
        Self { pool }
    }
}

impl GalleryRepository<'_> {
    // Interpolated identifiers are fixed here or selected from GallerySort's closed enum.
    const COLUMNS: &'static str = "id,title,layout,expires_at,included_photos,watermark_enabled,hero_id,share_token,shared_at,last_opened_at,delivered_at,archived_at,COALESCE(expires_at <= now(), false) AS expired,created_at,updated_at";

    /// How long after a recorded client open later opens change nothing. Fixed, not configurable:
    /// a minute collapses reload bursts while "last opened" stays fresh enough to compare with
    /// the shared and delivered times.
    pub const OPEN_THROTTLE: Duration = Duration::from_secs(60);

    /// Creates an untitled gallery with the default settings.
    pub async fn create(&self, workspace_id: WorkspaceId) -> Result<Gallery, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "INSERT INTO galleries(id,workspace_id) VALUES ($1,$2) RETURNING {}",
            Self::COLUMNS
        )))
        .bind(GalleryId::generate())
        .bind(workspace_id)
        .fetch_one(self.pool)
        .await?)
    }

    /// Lists the workspace's galleries that are in any of `states`; an empty slice lists all.
    /// The photo totals come from the same query, so they are never stale.
    pub async fn list(
        &self,
        workspace_id: WorkspaceId,
        sort: GallerySort,
        states: &[GalleryState],
    ) -> Result<Vec<GalleryOverview>, DatabaseError> {
        let filter = if states.is_empty() {
            String::new()
        } else {
            let conditions = states.iter().map(|state| state.condition());

            format!(" AND ({})", conditions.collect::<Vec<_>>().join(" OR "))
        };

        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {}, totals.photo_count, totals.favorite_count,
                    totals.final_count, totals.downloaded_count
             FROM galleries
             CROSS JOIN LATERAL (
                 SELECT count(*) FILTER (WHERE kind = 'proof') AS photo_count,
                        count(*) FILTER (WHERE kind = 'proof' AND favorited_at IS NOT NULL) AS favorite_count,
                        count(*) FILTER (WHERE kind = 'final') AS final_count,
                        count(*) FILTER (WHERE kind = 'final' AND downloaded_at IS NOT NULL)
                            AS downloaded_count
                 FROM photos
                 WHERE photos.gallery_id = galleries.id AND photos.status = 'ready'
             ) AS totals
             WHERE workspace_id=$1{filter} ORDER BY {}",
            Self::COLUMNS,
            sort.order_by()
        )))
        .bind(workspace_id)
        .fetch_all(self.pool)
        .await?)
    }

    pub async fn get(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Option<Gallery>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM galleries WHERE id=$1 AND workspace_id=$2",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Ends client access without deleting anything. Archiving again keeps the first time; it is
    /// not an edit, so `updated_at` is untouched. `None` if the workspace has no such gallery.
    pub async fn archive(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Option<Gallery>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE galleries SET archived_at = COALESCE(archived_at, now())
             WHERE id=$1 AND workspace_id=$2
             RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Switches the gallery's private link from proofs to the finished photos. Only a shared,
    /// unarchived gallery can be delivered; delivering again keeps the first time. Delivery is
    /// not an edit, so `updated_at` is untouched, and it never depends on the finals' status:
    /// clients only ever see the ready ones.
    ///
    /// The `UPDATE` takes the row lock that [`PhotoRepository::set_favorite`] waits on, so a
    /// favorite write either lands before delivery or is rejected after it.
    ///
    /// [`PhotoRepository::set_favorite`]: crate::photo::PhotoRepository::set_favorite
    pub async fn deliver(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Delivery, DatabaseError> {
        let delivered = sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE galleries SET delivered_at = COALESCE(delivered_at, now())
             WHERE id=$1 AND workspace_id=$2 AND shared_at IS NOT NULL AND archived_at IS NULL
             RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.pool)
        .await?;

        Ok(match delivered {
            Some(gallery) => Delivery::Delivered(gallery),
            None if self.get(workspace_id, id).await?.is_some() => Delivery::NotDeliverable,
            None => Delivery::NotFound,
        })
    }

    /// Undoes [`archive`](Self::archive) at the same link. Delivery, sharing and expiry are
    /// untouched, so a gallery that has expired stays expired.
    pub async fn restore(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Option<Gallery>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE galleries SET archived_at = NULL
             WHERE id=$1 AND workspace_id=$2
             RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Whether a gallery with this id exists in any workspace.
    pub async fn exists(&self, id: GalleryId) -> Result<bool, DatabaseError> {
        Ok(
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM galleries WHERE id=$1)")
                .bind(id)
                .fetch_one(self.pool)
                .await?,
        )
    }

    /// Returns the gallery with its private link, creating the link if it has none. A shared
    /// gallery keeps its token for good. Sharing is not an edit, so `updated_at` is untouched.
    pub async fn share(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Option<Gallery>, DatabaseError> {
        // The row lock taken by UPDATE makes concurrent calls converge on the first token.
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE galleries SET share_token = COALESCE(share_token, $3),
                 shared_at = COALESCE(shared_at, now())
             WHERE id=$1 AND workspace_id=$2
             RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .bind(ShareToken::generate())
        .fetch_optional(self.pool)
        .await?)
    }

    /// Finds the gallery behind a private link, with where its photos live. Unknown, archived and expired links are the same `None`,
    /// so a client cannot tell them apart; the database clock decides expiry.
    pub async fn find_shared(
        &self,
        token: &ShareToken,
    ) -> Result<Option<SharedAccess>, DatabaseError> {
        Ok(sqlx::query_as(
            "SELECT g.id AS gallery_id,g.workspace_id,g.title,g.layout,g.expires_at,g.included_photos, g.watermark_enabled,
                    g.delivered_at IS NOT NULL AS delivered,
                    (SELECT p.id FROM photos p
                     WHERE p.id = g.hero_id AND p.clean_ready AND p.kind = 'proof'
                       AND p.gallery_id = g.id AND p.workspace_id = g.workspace_id) AS hero_id
             FROM galleries g
             WHERE g.share_token=$1 AND g.archived_at IS NULL
               AND (g.expires_at IS NULL OR g.expires_at > now())",
        )
        .bind(token)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Records a client open of a gallery reached through its private link. A gallery opened less
    /// than [`Self::OPEN_THROTTLE`] ago keeps its time, which bounds the writes of reloads and
    /// repeat visits at the price of precision. An archived or expired gallery is not marked. Returns whether this
    /// call recorded the open.
    pub async fn record_opened(&self, location: PhotoLocation) -> Result<bool, DatabaseError> {
        let result = sqlx::query(
            "UPDATE galleries SET last_opened_at = now()
             WHERE id=$1 AND workspace_id=$2 AND archived_at IS NULL
               AND (expires_at IS NULL OR expires_at > now())
               AND (last_opened_at IS NULL OR last_opened_at < now() - make_interval(secs => $3))",
        )
        .bind(location.gallery_id)
        .bind(location.workspace_id)
        .bind(Self::OPEN_THROTTLE.as_secs_f64())
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() == 1)
    }

    /// Applies `changes` and returns the gallery, or `None` if the workspace has no such
    /// gallery or the requested hero is not one of its ready photos, in which case nothing
    /// changes. The title is trimmed, and a blank one clears it. An empty changeset only reads, so it does not
    /// count as an edit.
    pub async fn update(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
        changes: &GalleryChanges,
    ) -> Result<Option<Gallery>, GalleryError> {
        if changes.is_empty() {
            return Ok(self.get(workspace_id, id).await?);
        }

        Self::update_in(self.pool, workspace_id, id, changes).await
    }

    /// Applies a non-empty changeset through `executor`; see [`update`](Self::update).
    pub(crate) async fn update_in(
        executor: impl PgExecutor<'_>,
        workspace_id: WorkspaceId,
        id: GalleryId,
        changes: &GalleryChanges,
    ) -> Result<Option<Gallery>, GalleryError> {
        let included_photos = changes
            .included_photos
            .map(|count| {
                count
                    .map(|count| i32::try_from(count.get()))
                    .transpose()
                    .map_err(|_| GalleryError::LengthOverflow(GallerySetting::IncludedPhotos))
            })
            .transpose()?;

        let title = changes.title.as_ref().map(|title| {
            title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
        });

        sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE galleries SET
                title = CASE WHEN $3 THEN $4 ELSE title END,
                layout = COALESCE($5, layout),
                expires_at = CASE WHEN $6 THEN $7 ELSE expires_at END,
                included_photos = CASE WHEN $8 THEN $9 ELSE included_photos END,
                watermark_enabled = COALESCE($10, watermark_enabled),
                hero_id = CASE WHEN $11 THEN $12 ELSE hero_id END,
                updated_at = now()
             WHERE id=$1 AND workspace_id=$2
               AND (NOT $11 OR $12::uuid IS NULL OR EXISTS (
                   SELECT 1 FROM photos
                   WHERE id=$12 AND gallery_id=$1 AND workspace_id=$2 AND clean_ready
                     AND kind='proof'
                   FOR KEY SHARE))
             RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .bind(title.is_some())
        .bind(title.flatten())
        .bind(changes.layout)
        .bind(changes.expires_at.is_some())
        .bind(changes.expires_at.flatten())
        .bind(included_photos.is_some())
        .bind(included_photos.flatten())
        .bind(changes.watermark_enabled)
        .bind(changes.hero_id.is_some())
        .bind(changes.hero_id.flatten())
        .fetch_optional(executor)
        .await
        .map_err(GalleryError::classify)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::*;
    use crate::{
        Database,
        jobs::{JobOptions, JobPayload, JobRepository, ProcessPhoto},
        photo::{PhotoKind, PhotoRepository, PhotoStatus, Watermarked},
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

    fn changes(value: serde_json::Value) -> GalleryChanges {
        serde_json::from_value(value).unwrap()
    }

    async fn set_column(pool: &PgPool, id: GalleryId, column: &str, value: &str) {
        sqlx::query(AssertSqlSafe(format!(
            "UPDATE galleries SET {column} = {value} WHERE id = $1"
        )))
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn states_follow_the_timestamps(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();

        assert_eq!(gallery.states, [GalleryState::Draft]);

        let shared = store.share(workspace, gallery.id).await.unwrap().unwrap();

        assert_eq!(shared.states, [GalleryState::Shared]);

        set_column(&pool, gallery.id, "delivered_at", "now()").await;
        let delivered = store.get(workspace, gallery.id).await.unwrap().unwrap();

        assert_eq!(delivered.states, [GalleryState::Delivered]);

        set_column(&pool, gallery.id, "archived_at", "now()").await;
        let archived = store.get(workspace, gallery.id).await.unwrap().unwrap();

        assert_eq!(
            archived.states,
            [GalleryState::Delivered, GalleryState::Archived]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn expired_is_a_flag_on_any_progress_state(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();

        set_column(&pool, gallery.id, "expires_at", "now() + interval '1 day'").await;
        let future = store.get(workspace, gallery.id).await.unwrap().unwrap();

        assert_eq!(future.states, [GalleryState::Draft]);

        set_column(&pool, gallery.id, "expires_at", "now() - interval '1 day'").await;
        set_column(&pool, gallery.id, "archived_at", "now()").await;
        let past = store.get(workspace, gallery.id).await.unwrap().unwrap();

        assert_eq!(
            past.states,
            [
                GalleryState::Draft,
                GalleryState::Archived,
                GalleryState::Expired
            ]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn share_records_when_the_link_was_created(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();
        let first = store.share(workspace, gallery.id).await.unwrap().unwrap();
        let shared_at = |id: GalleryId| {
            let pool = pool.clone();

            async move {
                sqlx::query_scalar::<_, Option<Timestamp>>(
                    "SELECT shared_at FROM galleries WHERE id = $1",
                )
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
            }
        };
        let recorded = shared_at(gallery.id).await;

        assert!(recorded.is_some());

        store.share(workspace, gallery.id).await.unwrap();

        assert_eq!(shared_at(gallery.id).await, recorded);
        assert_eq!(first.updated_at, gallery.updated_at);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_backfills_shared_at(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("gallery_states_migration_{}", Uuid::new_v4().simple());

        sqlx::query(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("SET LOCAL search_path TO {schema}")))
            .execute(&mut *transaction)
            .await
            .unwrap();

        for migration in [
            include_str!("../migrations/0001_accounts_sessions.sql"),
            include_str!("../migrations/0002_galleries.sql"),
            include_str!("../migrations/0003_share_tokens.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO galleries(id, workspace_id, share_token, updated_at) VALUES
                 ($1, $3, 'AAAAAAAAAAAAAAAAAAAAAA', '2026-10-31 10:00:00+00'),
                 ($2, $3, NULL, '2026-10-31 11:00:00+00')",
        )
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind(workspace)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0008_gallery_states.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();

        let rows: Vec<(Option<Timestamp>, Timestamp)> =
            sqlx::query_as("SELECT shared_at, updated_at FROM galleries ORDER BY updated_at")
                .fetch_all(&mut *transaction)
                .await
                .unwrap();

        assert_eq!(rows[0].0, Some(rows[0].1));
        assert_eq!(rows[1].0, None);

        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn new_galleries_have_defaults_and_stay_in_their_workspace(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);
        let gallery = store.create(mine).await.unwrap();

        assert_eq!(gallery.title, None);
        assert_eq!(gallery.layout, GalleryLayout::Grid);
        assert_eq!(gallery.expires_at, None);
        assert_eq!(gallery.included_photos, None);
        assert!(!gallery.watermark_enabled);
        assert_eq!(
            store.get(mine, gallery.id).await.unwrap(),
            Some(gallery.clone())
        );
        assert_eq!(store.get(other, gallery.id).await.unwrap(), None);
        assert!(
            store
                .list(other, GallerySort::Updated, &[])
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            store
                .update(other, gallery.id, &changes(json!({"title": "Stolen"})))
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store.get(mine, gallery.id).await.unwrap().unwrap().title,
            None
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn changes_set_clear_and_keep_independently(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;

        let set = store
            .update(
                workspace,
                id,
                &changes(json!({
                    "title": "  Wedding  ",
                    "layout": "masonry",
                    "expires_at": "2030-01-02T03:04:05Z",
                    "included_photos": 25,
                    "watermark_enabled": true,
                })),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(set.title.as_deref(), Some("Wedding"));
        assert_eq!(set.layout, GalleryLayout::Masonry);
        assert!(set.expires_at.is_some());
        assert_eq!(set.included_photos, NonZeroUsize::new(25));
        assert!(set.watermark_enabled);

        let cleared = store
            .update(workspace, id, &changes(json!({"expires_at": null})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(cleared.expires_at, None);
        assert_eq!(cleared.included_photos, NonZeroUsize::new(25));
        assert_eq!(cleared.title.as_deref(), Some("Wedding"));

        let cleared = store
            .update(workspace, id, &changes(json!({"included_photos": null})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(cleared.included_photos, None);
        assert_eq!(cleared.layout, GalleryLayout::Masonry);

        for blank in [json!(null), json!(""), json!("   ")] {
            let untitled = store
                .update(workspace, id, &changes(json!({"title": blank})))
                .await
                .unwrap()
                .unwrap();

            assert_eq!(untitled.title, None);

            store
                .update(workspace, id, &changes(json!({"title": "Again"})))
                .await
                .unwrap();
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn invalid_titles_and_counts_are_rejected(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;
        let long = "é".repeat(256);

        let result = store
            .update(workspace, id, &changes(json!({"title": long})))
            .await;

        assert!(matches!(
            result,
            Err(GalleryError::LengthOverflow(GallerySetting::Title))
        ));

        let result = store
            .update(workspace, id, &changes(json!({"title": "nul\u{0}byte"})))
            .await;

        assert!(matches!(
            result,
            Err(GalleryError::InvalidCharacters(GallerySetting::Title))
        ));

        let widest = "é".repeat(255);
        let updated = store
            .update(workspace, id, &changes(json!({"title": widest})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.title.unwrap().chars().count(), 255);

        let oversized = GalleryChanges {
            included_photos: Some(NonZeroUsize::new(usize::MAX)),
            ..GalleryChanges::default()
        };

        assert!(matches!(
            store.update(workspace, id, &oversized).await,
            Err(GalleryError::LengthOverflow(GallerySetting::IncludedPhotos))
        ));
    }

    #[test]
    fn changeset_parsing_rejects_nulls_unknown_fields_and_bad_counts() {
        for body in [
            json!({"layout": null}),
            json!({"watermark_enabled": null}),
            json!({"included_photos": 0}),
            json!({"layout": "carousel"}),
            json!({"colour": "red"}),
            json!({"expires_at": "9999-12-31T23:59:59-01:00"}),
            json!({"expires_at": "0000-01-01T00:00:00+01:00"}),
        ] {
            assert!(
                serde_json::from_value::<GalleryChanges>(body.clone()).is_err(),
                "{body}"
            );
        }

        assert!(changes(json!({})).is_empty());
        assert!(!changes(json!({"included_photos": null})).is_empty());
        assert!(!changes(json!({"title": null})).is_empty());

        let partial = changes(json!({"title": null, "watermark_enabled": true}));

        assert_eq!(
            serde_json::to_value(&partial).unwrap(),
            json!({"title": null, "watermark_enabled": true})
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn empty_changeset_does_not_count_as_an_edit(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();
        let unchanged = store
            .update(workspace, gallery.id, &GalleryChanges::default())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(unchanged, gallery);

        let edited = store
            .update(
                workspace,
                gallery.id,
                &changes(json!({"watermark_enabled": true})),
            )
            .await
            .unwrap()
            .unwrap();

        assert!(edited.updated_at > gallery.updated_at);
        assert_eq!(edited.created_at, gallery.created_at);
    }

    async fn photo_in(
        pool: &PgPool,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        status: PhotoStatus,
    ) -> PhotoId {
        let id = PhotoId::from(Uuid::new_v4());
        let status = match status {
            PhotoStatus::Waiting => "waiting",
            PhotoStatus::Processing => "processing",
            PhotoStatus::Ready => "ready",
            PhotoStatus::Failed(_) => unreachable!("failed photos need a reason"),
        };

        sqlx::query(
            "INSERT INTO photos(id, workspace_id, gallery_id, original_name, size, width, height, status, clean_ready, watermark_revision)
             VALUES ($1, $2, $3, 'p.jpg', 1, 1, 1, $4::photo_status, $4 = 'ready', CASE WHEN $4 = 'ready' THEN 0 END)",
        )
        .bind(id)
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(status)
        .execute(pool)
        .await
        .unwrap();

        id
    }

    fn hero(id: Option<PhotoId>) -> GalleryChanges {
        GalleryChanges {
            hero_id: Some(id),
            ..GalleryChanges::default()
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn hero_is_set_changed_and_cleared_by_ready_photos_of_the_gallery(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();
        let (first, second) = (
            photo_in(&pool, workspace, gallery.id, PhotoStatus::Ready).await,
            photo_in(&pool, workspace, gallery.id, PhotoStatus::Ready).await,
        );

        assert_eq!(gallery.hero_id, None);

        for expected in [Some(first), Some(second), None] {
            let updated = store
                .update(workspace, gallery.id, &hero(expected))
                .await
                .unwrap()
                .unwrap();

            assert_eq!(updated.hero_id, expected);
        }

        store
            .update(workspace, gallery.id, &hero(Some(first)))
            .await
            .unwrap();

        let kept = store
            .update(workspace, gallery.id, &changes(json!({"title": "Kept"})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(kept.hero_id, Some(first));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn hero_rejects_foreign_unknown_and_unready_photos(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);
        let gallery = store.create(mine).await.unwrap();
        let sibling = store.create(mine).await.unwrap();
        let foreign_gallery = store.create(other).await.unwrap();
        let ready = photo_in(&pool, mine, gallery.id, PhotoStatus::Ready).await;
        let candidates = [
            photo_in(&pool, mine, sibling.id, PhotoStatus::Ready).await,
            photo_in(&pool, other, foreign_gallery.id, PhotoStatus::Ready).await,
            photo_in(&pool, mine, gallery.id, PhotoStatus::Waiting).await,
            photo_in(&pool, mine, gallery.id, PhotoStatus::Processing).await,
            PhotoId::from(Uuid::new_v4()),
        ];

        store
            .update(mine, gallery.id, &hero(Some(ready)))
            .await
            .unwrap();

        for candidate in candidates {
            let result = store
                .update(mine, gallery.id, &hero(Some(candidate)))
                .await
                .unwrap();

            assert_eq!(result, None);
        }

        let current = store.get(mine, gallery.id).await.unwrap().unwrap();

        assert_eq!(current.hero_id, Some(ready));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_the_hero_photo_clears_the_hero(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();
        let (hero_photo, other) = (
            photo_in(&pool, workspace, gallery.id, PhotoStatus::Ready).await,
            photo_in(&pool, workspace, gallery.id, PhotoStatus::Ready).await,
        );

        store
            .update(workspace, gallery.id, &hero(Some(hero_photo)))
            .await
            .unwrap();
        database
            .repository::<WorkspaceDeletion>()
            .delete_photos(workspace, gallery.id, PhotoKind::Proof, &[hero_photo])
            .await
            .unwrap();

        let current = store.get(workspace, gallery.id).await.unwrap().unwrap();

        assert_eq!(current.hero_id, None);
        assert!(
            database
                .repository::<PhotoRepository>()
                .get(workspace, gallery.id, other, PhotoKind::Proof)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn hero_can_change_after_sharing_and_clients_see_it_only_while_ready(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;
        let (first, second) = (
            photo_in(&pool, workspace, id, PhotoStatus::Ready).await,
            photo_in(&pool, workspace, id, PhotoStatus::Ready).await,
        );
        let token = store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();

        for expected in [Some(first), Some(second)] {
            store.update(workspace, id, &hero(expected)).await.unwrap();

            let shared = store.find_shared(&token).await.unwrap().unwrap();

            assert_eq!(shared.gallery.hero_id, expected);
        }

        sqlx::query("UPDATE photos SET status = 'processing', clean_ready = false WHERE id = $1")
            .bind(second)
            .execute(&pool)
            .await
            .unwrap();

        let shared = store.find_shared(&token).await.unwrap().unwrap();

        assert_eq!(shared.gallery.hero_id, None);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn lists_follow_the_requested_sort(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let mut ids = Vec::new();

        for title in ["banana", "Apple", "cherry"] {
            let gallery = store.create(workspace).await.unwrap();

            store
                .update(workspace, gallery.id, &changes(json!({"title": title})))
                .await
                .unwrap();
            ids.push(gallery.id);
        }

        // The oldest gallery becomes the most recently edited.
        store
            .update(
                workspace,
                ids[0],
                &changes(json!({"watermark_enabled": true})),
            )
            .await
            .unwrap();

        let titles = |sort| async move {
            store
                .list(workspace, sort, &[])
                .await
                .unwrap()
                .into_iter()
                .map(|overview| overview.gallery.title.unwrap_or_default())
                .collect::<Vec<_>>()
        };

        assert_eq!(
            titles(GallerySort::Updated).await,
            ["banana", "cherry", "Apple"]
        );
        assert_eq!(
            titles(GallerySort::Created).await,
            ["cherry", "Apple", "banana"]
        );
        assert_eq!(
            titles(GallerySort::Oldest).await,
            ["banana", "Apple", "cherry"]
        );
        assert_eq!(
            titles(GallerySort::Title).await,
            ["Apple", "banana", "cherry"]
        );

        store.create(workspace).await.unwrap();

        assert_eq!(
            titles(GallerySort::Title).await,
            ["Apple", "banana", "cherry", ""]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn list_counts_ready_photos_and_favorites_per_gallery(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let (empty, busy, other) = (
            store.create(workspace).await.unwrap(),
            store.create(workspace).await.unwrap(),
            store.create(workspace).await.unwrap(),
        );
        let ready = [
            photo_in(&pool, workspace, busy.id, PhotoStatus::Ready).await,
            photo_in(&pool, workspace, busy.id, PhotoStatus::Ready).await,
            photo_in(&pool, workspace, busy.id, PhotoStatus::Ready).await,
        ];
        photo_in(&pool, workspace, busy.id, PhotoStatus::Processing).await;
        photo_in(&pool, workspace, other.id, PhotoStatus::Ready).await;

        let location = PhotoLocation {
            workspace_id: workspace,
            gallery_id: busy.id,
        };

        for id in &ready[..2] {
            database
                .repository::<PhotoRepository>()
                .set_favorite(location, *id, true, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap();
        }

        let counts = store
            .list(workspace, GallerySort::Title, &[])
            .await
            .unwrap()
            .into_iter()
            .map(|overview| {
                (
                    overview.gallery.id,
                    (overview.photo_count, overview.favorite_count),
                )
            })
            .collect::<HashMap<_, _>>();

        assert_eq!(counts[&empty.id], (0, 0));
        assert_eq!(counts[&busy.id], (3, 2));
        assert_eq!(counts[&other.id], (1, 0));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn list_counts_the_ready_finals_and_their_downloads(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();

        for (status, downloaded) in [("ready", true), ("ready", false), ("processing", true)] {
            sqlx::query(
                "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height,
                                     kind, status, downloaded_at)
                 VALUES (gen_random_uuid(), $1, $2, 'a.jpg', 1, 1, 1, 'final', $3::photo_status,
                         CASE WHEN $4 THEN now() END)",
            )
            .bind(workspace)
            .bind(gallery.id)
            .bind(status)
            .bind(downloaded)
            .execute(&pool)
            .await
            .unwrap();
        }

        let overview = &store
            .list(workspace, GallerySort::Title, &[])
            .await
            .unwrap()[0];

        assert_eq!(
            (
                overview.photo_count,
                overview.final_count,
                overview.downloaded_count
            ),
            (0, 2, 1)
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn list_filters_by_any_chosen_state(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let mut ids = Vec::new();

        for title in ["draft", "shared", "delivered", "archived", "expired"] {
            let gallery = store.create(workspace).await.unwrap();

            store
                .update(workspace, gallery.id, &changes(json!({"title": title})))
                .await
                .unwrap();
            ids.push(gallery.id);
        }

        store.share(workspace, ids[1]).await.unwrap();
        set_column(&pool, ids[2], "shared_at", "now()").await;
        set_column(&pool, ids[2], "delivered_at", "now()").await;
        set_column(&pool, ids[3], "archived_at", "now()").await;
        set_column(&pool, ids[4], "expires_at", "now() - interval '1 day'").await;

        let titles = |sort, states: &'static [GalleryState]| async move {
            store
                .list(workspace, sort, states)
                .await
                .unwrap()
                .into_iter()
                .map(|overview| overview.gallery.title.unwrap_or_default())
                .collect::<Vec<_>>()
        };

        use GalleryState::{Archived, Delivered, Draft, Expired, Shared};

        assert_eq!(titles(GallerySort::Title, &[]).await.len(), 5);
        assert_eq!(titles(GallerySort::Title, &[Shared]).await, ["shared"]);
        assert_eq!(
            titles(GallerySort::Title, &[Shared, Shared]).await,
            ["shared"]
        );
        assert_eq!(
            titles(GallerySort::Title, &[Delivered, Archived]).await,
            ["archived", "delivered"]
        );
        assert_eq!(
            titles(GallerySort::Title, &[Draft, Expired]).await,
            ["archived", "draft", "expired"]
        );
        assert_eq!(titles(GallerySort::Created, &[Expired]).await, ["expired"]);
        assert_eq!(
            titles(GallerySort::Updated, &[Archived]).await,
            ["archived"]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn list_filter_stays_in_the_workspace(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);

        store.create(mine).await.unwrap();

        for state in [
            GalleryState::Draft,
            GalleryState::Shared,
            GalleryState::Delivered,
            GalleryState::Archived,
            GalleryState::Expired,
        ] {
            assert!(
                store
                    .list(other, GallerySort::Updated, &[state])
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn sharing_creates_one_fixed_token_per_gallery(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let gallery = store.create(workspace).await.unwrap();

        assert_eq!(gallery.share_token, None);

        let shared = store.share(workspace, gallery.id).await.unwrap().unwrap();
        let token = shared.share_token.clone().unwrap();

        assert_eq!(shared.updated_at, gallery.updated_at);
        assert_eq!(
            store
                .share(workspace, gallery.id)
                .await
                .unwrap()
                .unwrap()
                .share_token,
            Some(token.clone())
        );
        assert_eq!(
            store
                .get(workspace, gallery.id)
                .await
                .unwrap()
                .unwrap()
                .share_token,
            Some(token)
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn concurrent_shares_converge_on_one_token(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;

        let (a, b, c, d) = tokio::join!(
            store.share(workspace, id),
            store.share(workspace, id),
            store.share(workspace, id),
            store.share(workspace, id),
        );
        let tokens: Vec<_> = [a, b, c, d]
            .into_iter()
            .map(|gallery| gallery.unwrap().unwrap().share_token.unwrap())
            .collect();

        assert!(tokens.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn another_workspace_cannot_share(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);
        let id = store.create(mine).await.unwrap().id;

        assert_eq!(store.share(other, id).await.unwrap(), None);
        assert_eq!(
            store.get(mine, id).await.unwrap().unwrap().share_token,
            None
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn record_opened_refreshes_only_after_the_throttle(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;
        let token = store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let location = store.find_shared(&token).await.unwrap().unwrap().location;
        let last_opened_at = || async {
            store
                .get(workspace, id)
                .await
                .unwrap()
                .unwrap()
                .last_opened_at
        };

        assert_eq!(last_opened_at().await, None);
        assert!(store.record_opened(location).await.unwrap());

        let first = last_opened_at().await;

        assert!(first.is_some());
        assert!(!store.record_opened(location).await.unwrap());
        assert_eq!(last_opened_at().await, first);

        let age = |seconds: f64| {
            sqlx::query("UPDATE galleries SET last_opened_at = now() - make_interval(secs => $1)")
                .bind(seconds)
                .execute(&pool)
        };

        age(30.0).await.unwrap();

        let within = last_opened_at().await;

        assert!(!store.record_opened(location).await.unwrap());
        assert_eq!(last_opened_at().await, within);

        age(61.0).await.unwrap();

        let aged = last_opened_at().await;

        assert!(store.record_opened(location).await.unwrap());
        assert!(last_opened_at().await > aged);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn record_opened_ignores_archived_and_expired_galleries(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let mut locations = Vec::new();

        for _ in 0..2 {
            let id = store.create(workspace).await.unwrap().id;
            let token = store
                .share(workspace, id)
                .await
                .unwrap()
                .unwrap()
                .share_token
                .unwrap();

            locations.push((
                id,
                store.find_shared(&token).await.unwrap().unwrap().location,
            ));
        }

        store.archive(workspace, locations[0].0).await.unwrap();
        store
            .update(
                workspace,
                locations[1].0,
                &changes(json!({"expires_at": "2000-01-01T00:00:00Z"})),
            )
            .await
            .unwrap();

        for (id, location) in locations {
            assert!(!store.record_opened(location).await.unwrap());
            assert_eq!(
                store
                    .get(workspace, id)
                    .await
                    .unwrap()
                    .unwrap()
                    .last_opened_at,
                None
            );
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn find_shared_ignores_unknown_and_unshared(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;

        store.create(workspace).await.unwrap();

        assert_eq!(
            store.find_shared(&ShareToken::generate()).await.unwrap(),
            None
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn find_shared_enforces_expiry_at_request_time(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;

        store
            .update(
                workspace,
                id,
                &changes(json!({"title": "Wedding", "layout": "masonry"})),
            )
            .await
            .unwrap();

        let token = store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let set_expiry = |value: serde_json::Value| {
            let store = &store;

            async move {
                store
                    .update(workspace, id, &changes(json!({"expires_at": value})))
                    .await
                    .unwrap();
            }
        };

        let found = store.find_shared(&token).await.unwrap().unwrap();

        assert_eq!(
            found.location,
            PhotoLocation {
                workspace_id: workspace,
                gallery_id: id
            }
        );
        assert_eq!(found.gallery.title.as_deref(), Some("Wedding"));
        assert_eq!(found.gallery.layout, GalleryLayout::Masonry);
        assert_eq!(found.gallery.expires_at, None);

        set_expiry(json!("2000-01-01T00:00:00Z")).await;

        assert_eq!(store.find_shared(&token).await.unwrap(), None);

        set_expiry(json!("2999-01-01T00:00:00Z")).await;

        assert!(
            store
                .find_shared(&token)
                .await
                .unwrap()
                .unwrap()
                .gallery
                .expires_at
                .is_some()
        );

        set_expiry(json!("2000-01-01T00:00:00Z")).await;
        set_expiry(serde_json::Value::Null).await;

        assert!(store.find_shared(&token).await.unwrap().is_some());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn find_shared_hides_archived_galleries_and_reports_delivery_and_allowance(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;

        store
            .update(workspace, id, &changes(json!({"included_photos": 10})))
            .await
            .unwrap();

        let token = store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let found = store.find_shared(&token).await.unwrap().unwrap().gallery;

        assert_eq!(found.included_photos, NonZeroUsize::new(10));
        assert!(!found.delivered);

        set_column(&pool, id, "delivered_at", "now()").await;

        assert!(
            store
                .find_shared(&token)
                .await
                .unwrap()
                .unwrap()
                .gallery
                .delivered
        );

        set_column(&pool, id, "archived_at", "now()").await;

        assert_eq!(store.find_shared(&token).await.unwrap(), None);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deliver_needs_a_shared_unarchived_gallery_and_is_idempotent(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;

        assert!(matches!(
            store.deliver(workspace, id).await.unwrap(),
            Delivery::NotDeliverable
        ));
        assert!(matches!(
            store
                .deliver(workspace, GalleryId(Uuid::new_v4()))
                .await
                .unwrap(),
            Delivery::NotFound
        ));
        assert!(matches!(
            store
                .deliver(self::workspace(&pool).await, id)
                .await
                .unwrap(),
            Delivery::NotFound
        ));

        store.share(workspace, id).await.unwrap();

        let Delivery::Delivered(first) = store.deliver(workspace, id).await.unwrap() else {
            panic!("a shared gallery can be delivered");
        };
        let Delivery::Delivered(second) = store.deliver(workspace, id).await.unwrap() else {
            panic!("delivering again keeps the delivery");
        };

        assert_eq!(first.states, [GalleryState::Delivered]);
        assert_eq!(first.updated_at, second.updated_at);
        assert_eq!(first.states, second.states);

        store.archive(workspace, id).await.unwrap();

        assert!(matches!(
            store.deliver(workspace, id).await.unwrap(),
            Delivery::NotDeliverable
        ));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn archive_and_restore_round_trip_and_are_idempotent(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let created = store.create(workspace).await.unwrap();
        let token = store
            .share(workspace, created.id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();

        let archived = store.archive(workspace, created.id).await.unwrap().unwrap();
        let again = store.archive(workspace, created.id).await.unwrap().unwrap();

        assert_eq!(
            archived.states,
            [GalleryState::Shared, GalleryState::Archived]
        );
        assert_eq!(archived, again);
        assert_eq!(archived.updated_at, created.updated_at);
        assert_eq!(store.find_shared(&token).await.unwrap(), None);

        let restored = store.restore(workspace, created.id).await.unwrap().unwrap();
        let restored_again = store.restore(workspace, created.id).await.unwrap().unwrap();

        assert_eq!(restored.states, [GalleryState::Shared]);
        assert_eq!(restored, restored_again);
        assert!(store.find_shared(&token).await.unwrap().is_some());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn restore_keeps_delivery_and_expiry(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;

        store.share(workspace, id).await.unwrap();
        set_column(&pool, id, "delivered_at", "now()").await;
        set_column(&pool, id, "expires_at", "now() - interval '1 day'").await;
        store.archive(workspace, id).await.unwrap();

        let restored = store.restore(workspace, id).await.unwrap().unwrap();

        assert_eq!(
            restored.states,
            [GalleryState::Delivered, GalleryState::Expired]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn archive_restore_and_delete_ignore_other_workspaces(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let owner = workspace(&pool).await;
        let other = workspace(&pool).await;
        let id = store.create(owner).await.unwrap().id;

        assert_eq!(store.archive(other, id).await.unwrap(), None);
        assert_eq!(store.restore(other, id).await.unwrap(), None);
        assert!(
            !database
                .repository::<WorkspaceDeletion>()
                .delete_gallery(other, id)
                .await
                .unwrap()
        );
        assert!(store.exists(id).await.unwrap());
        assert!(
            database
                .repository::<TestRepository>()
                .jobs_of_kind("cleanup_gallery")
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delete_removes_the_gallery_and_queues_one_cleanup_with_it(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;
        let photo = photo_in(&pool, workspace, id, PhotoStatus::Waiting).await;

        database
            .repository::<JobRepository>()
            .enqueue(
                JobPayload::ProcessPhoto(ProcessPhoto { photo_id: photo }),
                JobOptions::default().max_attempts,
            )
            .await
            .unwrap();

        assert!(
            database
                .repository::<WorkspaceDeletion>()
                .delete_gallery(workspace, id)
                .await
                .unwrap()
        );
        assert!(
            !database
                .repository::<WorkspaceDeletion>()
                .delete_gallery(workspace, id)
                .await
                .unwrap()
        );
        assert_eq!(store.get(workspace, id).await.unwrap(), None);
        assert!(!store.exists(id).await.unwrap());

        let tests = database.repository::<TestRepository>();
        let cleanups = tests.jobs_of_kind("cleanup_gallery").await.unwrap();

        assert_eq!(cleanups.len(), 1);
        assert_eq!(
            cleanups[0].payload["payload"],
            json!({"workspace_id": workspace.0, "gallery_id": id.0})
        );
        assert!(tests.photo_jobs(photo).await.unwrap().is_empty());
        assert_eq!(
            database
                .repository::<PhotoRepository>()
                .locate(photo)
                .await
                .unwrap(),
            None
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn share_token_check_rejects_malformed_values(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;

        let result = sqlx::query("UPDATE galleries SET share_token = 'short' WHERE id = $1")
            .bind(id)
            .execute(&pool)
            .await;

        assert!(result.is_err());
    }
}
