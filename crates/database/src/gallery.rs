//! Galleries owned by a workspace.
//!
//! Every query is scoped by `workspace_id`, so a gallery of another workspace is
//! indistinguishable from a missing one. Settings are edited through a [`GalleryChanges`]
//! changeset: a missing field keeps the stored value and, for optional settings, an explicit
//! `null` clears it. A gallery has no title until one is set; the default text belongs to the
//! front end, which knows the viewer's language. Value limits live in the database as CHECK constraints, which are the
//! single authority; violations surface as [`GalleryError`] variants.

use std::num::NonZeroUsize;

use derive_more::Display;
use pp_core::types::{generate::Generate, timestamp::Timestamp};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sqlx::{AssertSqlSafe, FromRow, PgPool, Row, Type, postgres::PgRow};
use thiserror::Error;
use uuid::Uuid;

pub use self::share::{ShareToken, SharedGallery};
use crate::{Repository, auth::WorkspaceId, error::DatabaseError};

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
    /// `None` means the gallery has not been shared.
    pub share_token: Option<ShareToken>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl FromRow<'_, PgRow> for Gallery {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let included_photos = row
            .try_get::<Option<i32>, _>("included_photos")?
            .map(|count| {
                usize::try_from(count)
                    .ok()
                    .and_then(NonZeroUsize::new)
                    .ok_or_else(|| sqlx::Error::Decode("included_photos is not positive".into()))
            })
            .transpose()?;

        Ok(Self {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            layout: row.try_get("layout")?,
            expires_at: row.try_get("expires_at")?,
            included_photos,
            watermark_enabled: row.try_get("watermark_enabled")?,
            share_token: row.try_get("share_token")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
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
    /// Alphabetical, ignoring case.
    Title,
}

impl GallerySort {
    fn order_by(self) -> &'static str {
        match self {
            Self::Updated => "updated_at DESC, id",
            Self::Created => "created_at DESC, id",
            Self::Title => "lower(title) NULLS LAST, id",
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
    const COLUMNS: &'static str = "id,title,layout,expires_at,included_photos,watermark_enabled,share_token,created_at,updated_at";

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

    pub async fn list(
        &self,
        workspace_id: WorkspaceId,
        sort: GallerySort,
    ) -> Result<Vec<Gallery>, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM galleries WHERE workspace_id=$1 ORDER BY {}",
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

    /// Returns the gallery with its private link, creating the link if it has none. A shared
    /// gallery keeps its token for good. Sharing is not an edit, so `updated_at` is untouched.
    pub async fn share(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Option<Gallery>, DatabaseError> {
        // The row lock taken by UPDATE makes concurrent calls converge on the first token.
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE galleries SET share_token = COALESCE(share_token, $3)
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

    /// Finds the gallery behind a private link. Unknown and expired links are the same `None`,
    /// so a client cannot tell them apart; the database clock decides expiry.
    pub async fn find_shared(
        &self,
        token: &ShareToken,
    ) -> Result<Option<SharedGallery>, DatabaseError> {
        Ok(sqlx::query_as(
            "SELECT title,layout,expires_at FROM galleries
             WHERE share_token=$1 AND (expires_at IS NULL OR expires_at > now())",
        )
        .bind(token)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Applies `changes` and returns the gallery, or `None` if the workspace has no such
    /// gallery. The title is trimmed, and a blank one clears it. An empty changeset only reads, so it does not
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
                updated_at = now()
             WHERE id=$1 AND workspace_id=$2
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
        .fetch_optional(self.pool)
        .await
        .map_err(GalleryError::classify)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::Database;

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
                .list(other, GallerySort::Updated)
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
                .list(workspace, sort)
                .await
                .unwrap()
                .into_iter()
                .map(|gallery| gallery.title.unwrap_or_default())
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

        assert_eq!(found.title.as_deref(), Some("Wedding"));
        assert_eq!(found.layout, GalleryLayout::Masonry);
        assert_eq!(found.expires_at, None);

        set_expiry(json!("2000-01-01T00:00:00Z")).await;

        assert_eq!(store.find_shared(&token).await.unwrap(), None);

        set_expiry(json!("2999-01-01T00:00:00Z")).await;

        assert!(
            store
                .find_shared(&token)
                .await
                .unwrap()
                .unwrap()
                .expires_at
                .is_some()
        );

        set_expiry(json!("2000-01-01T00:00:00Z")).await;
        set_expiry(serde_json::Value::Null).await;

        assert!(store.find_shared(&token).await.unwrap().is_some());
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
