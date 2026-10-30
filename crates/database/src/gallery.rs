//! Galleries owned by a workspace.
//!
//! Every query is scoped by `workspace_id`, so a gallery of another workspace is
//! indistinguishable from a missing one. Settings are edited through a [`GalleryChanges`]
//! changeset: a missing field keeps the stored value and, for optional settings, an explicit
//! `null` clears it. Value limits live in the database as CHECK constraints, which are the
//! single authority; violations surface as [`GalleryError::Invalid`].

use std::num::NonZeroUsize;

use derive_more::{Display, Error, From};
use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sqlx::{FromRow, PgPool, Row, Type, postgres::PgRow};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{Repository, auth::WorkspaceId};

/// Title given to a gallery created without any input.
pub const DEFAULT_TITLE: &str = "Untitled gallery";

/// Postgres SQLSTATE for a string containing a NUL byte.
const INVALID_BYTE_SEQUENCE: &str = "22021";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct GalleryId(pub Uuid);

/// Default arrangement of photos for viewers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "gallery_layout", rename_all = "lowercase")]
pub enum Layout {
    #[default]
    Grid,
    Masonry,
}

/// A point in time that is read and written as an RFC 3339 string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Timestamp(#[serde(with = "time::serde::rfc3339")] pub OffsetDateTime);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Gallery {
    pub id: GalleryId,
    pub title: String,
    pub layout: Layout,
    /// `None` means the gallery never expires.
    pub expires_at: Option<Timestamp>,
    /// `None` means selection is unlimited.
    pub included_photos: Option<NonZeroUsize>,
    pub watermark_enabled: bool,
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
#[derive(Debug, Default, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GalleryChanges {
    #[serde(default, deserialize_with = "supplied")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "supplied")]
    pub layout: Option<Layout>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub expires_at: Option<Option<Timestamp>>,
    #[serde(default, with = "serde_with::rust::double_option")]
    pub included_photos: Option<Option<NonZeroUsize>>,
    #[serde(default, deserialize_with = "supplied")]
    pub watermark_enabled: Option<bool>,
}

impl GalleryChanges {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Deserializes a present value as `Some`, failing on `null` instead of treating it as absent.
fn supplied<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
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
            Self::Title => "lower(title), id",
        }
    }
}

/// A setting that failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum Setting {
    #[display("title")]
    Title,
    #[display("included_photos")]
    IncludedPhotos,
}

#[derive(Debug, Display, Error, From)]
pub enum GalleryError {
    #[display("invalid {_0}")]
    #[from(skip)]
    Invalid(#[error(not(source))] Setting),
    #[display("database error: {_0}")]
    Database(sqlx::Error),
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

const COLUMNS: &str =
    "id,title,layout,expires_at,included_photos,watermark_enabled,created_at,updated_at";

impl GalleryRepository<'_> {
    /// Creates a gallery with the default title and settings.
    pub async fn create(&self, workspace_id: WorkspaceId) -> Result<Gallery, sqlx::Error> {
        sqlx::query_as(&format!(
            "INSERT INTO galleries(id,workspace_id,title) VALUES ($1,$2,$3) RETURNING {COLUMNS}"
        ))
        .bind(GalleryId(Uuid::new_v4()))
        .bind(workspace_id)
        .bind(DEFAULT_TITLE)
        .fetch_one(self.pool)
        .await
    }

    pub async fn list(
        &self,
        workspace_id: WorkspaceId,
        sort: GallerySort,
    ) -> Result<Vec<Gallery>, sqlx::Error> {
        sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM galleries WHERE workspace_id=$1 ORDER BY {}",
            sort.order_by()
        ))
        .bind(workspace_id)
        .fetch_all(self.pool)
        .await
    }

    pub async fn get(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Option<Gallery>, sqlx::Error> {
        sqlx::query_as(&format!(
            "SELECT {COLUMNS} FROM galleries WHERE id=$1 AND workspace_id=$2"
        ))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(self.pool)
        .await
    }

    /// Applies `changes` and returns the gallery, or `None` if the workspace has no such
    /// gallery. The title is trimmed first. An empty changeset only reads, so it does not
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
                    .map_err(|_| GalleryError::Invalid(Setting::IncludedPhotos))
            })
            .transpose()?;

        sqlx::query_as(&format!(
            "UPDATE galleries SET
                title = COALESCE($3, title),
                layout = COALESCE($4, layout),
                expires_at = CASE WHEN $5 THEN $6 ELSE expires_at END,
                included_photos = CASE WHEN $7 THEN $8 ELSE included_photos END,
                watermark_enabled = COALESCE($9, watermark_enabled),
                updated_at = now()
             WHERE id=$1 AND workspace_id=$2
             RETURNING {COLUMNS}"
        ))
        .bind(id)
        .bind(workspace_id)
        .bind(changes.title.as_deref().map(str::trim))
        .bind(changes.layout)
        .bind(changes.expires_at.is_some())
        .bind(changes.expires_at.flatten())
        .bind(included_photos.is_some())
        .bind(included_photos.flatten())
        .bind(changes.watermark_enabled)
        .fetch_optional(self.pool)
        .await
        .map_err(invalid_setting)
    }
}

/// Maps a validation failure to the setting that caused it; anything else stays a database error.
fn invalid_setting(error: sqlx::Error) -> GalleryError {
    let setting =
        error
            .as_database_error()
            .and_then(|db| match (db.constraint(), db.code().as_deref()) {
                (Some("galleries_title_length"), _) | (_, Some(INVALID_BYTE_SEQUENCE)) => {
                    Some(Setting::Title)
                }
                (Some("galleries_included_photos_positive"), _) => Some(Setting::IncludedPhotos),
                _ => None,
            });

    setting.map_or(GalleryError::Database(error), GalleryError::Invalid)
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

        assert_eq!(gallery.title, DEFAULT_TITLE);
        assert_eq!(gallery.layout, Layout::Grid);
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
            DEFAULT_TITLE
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

        assert_eq!(set.title, "Wedding");
        assert_eq!(set.layout, Layout::Masonry);
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
        assert_eq!(cleared.title, "Wedding");

        let cleared = store
            .update(workspace, id, &changes(json!({"included_photos": null})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(cleared.included_photos, None);
        assert_eq!(cleared.layout, Layout::Masonry);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn invalid_titles_and_counts_are_rejected(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;
        let long = "é".repeat(256);

        for title in ["", "   ", long.as_str(), "nul\u{0}byte"] {
            let result = store
                .update(workspace, id, &changes(json!({"title": title})))
                .await;

            assert!(
                matches!(result, Err(GalleryError::Invalid(Setting::Title))),
                "{title:?}"
            );
        }

        let widest = "é".repeat(255);
        let updated = store
            .update(workspace, id, &changes(json!({"title": widest})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.title.chars().count(), 255);

        let oversized = GalleryChanges {
            included_photos: Some(NonZeroUsize::new(usize::MAX)),
            ..GalleryChanges::default()
        };

        assert!(matches!(
            store.update(workspace, id, &oversized).await,
            Err(GalleryError::Invalid(Setting::IncludedPhotos))
        ));
    }

    #[test]
    fn changeset_parsing_rejects_nulls_unknown_fields_and_bad_counts() {
        for body in [
            json!({"title": null}),
            json!({"layout": null}),
            json!({"watermark_enabled": null}),
            json!({"included_photos": 0}),
            json!({"layout": "carousel"}),
            json!({"colour": "red"}),
        ] {
            assert!(
                serde_json::from_value::<GalleryChanges>(body.clone()).is_err(),
                "{body}"
            );
        }

        assert!(changes(json!({})).is_empty());
        assert!(!changes(json!({"included_photos": null})).is_empty());
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
                .map(|gallery| gallery.title)
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
    }
}
