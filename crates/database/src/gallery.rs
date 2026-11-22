//! Galleries owned by a workspace.
//!
//! Every query is scoped by `workspace_id`, so a gallery of another workspace is
//! indistinguishable from a missing one. Settings are edited through a [`GalleryChanges`]
//! changeset: a missing field keeps the stored value and, for optional settings, an explicit
//! `null` clears it. A gallery has no title until one is set; the default text belongs to the
//! front end, which knows the viewer's language. Value limits live in the database as CHECK constraints, which are the
//! single authority; violations surface as [`GalleryError`] variants.

use std::{collections::HashSet, num::NonZeroUsize, time::Duration};

use derive_more::Display;
use pp_core::email::{Email, EmailMessage, EmailSubject};
pub use pp_core::types::access_code::{
    ACCESS_CODE_DIGITS, GalleryAccessCode, ParseAccessCodeError,
};
use pp_core::types::{generate::Generate, timestamp::Timestamp};
use serde::{Deserialize, Serialize};
use sqlx::{
    AssertSqlSafe, FromRow, PgExecutor, PgPool, Postgres, Row, Transaction, Type, postgres::PgRow,
};
use thiserror::Error;
use time::Date;
use uuid::Uuid;

pub(crate) use self::share::TokenAttempt;
pub use self::share::{ShareToken, SharedAccess, SharedGallery, SharedGalleryGate};
use crate::{
    Repository,
    activity::{ActivityKind, ActivityOptions, ActivityRecorder},
    auth::WorkspaceId,
    contact::{ContactName, ContactNameExt, ContactRepository},
    error::{DatabaseError, UniqueViolation},
    jobs::{EmailRecipient, InvitationStage, Job, JobOptions, JobRepository, SendEmail},
    patch::Present,
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
    #[error("access code cannot be changed once the gallery is shared")]
    AccessCodeLocked,
    #[error("database error")]
    Database(#[from] DatabaseError),
}

impl GalleryError {
    /// The setting that caused a validation failure, if it was one.
    pub fn setting(&self) -> Option<GallerySetting> {
        match self {
            Self::LengthOverflow(setting) | Self::InvalidCharacters(setting) => Some(*setting),
            Self::AccessCodeLocked | Self::Database(_) => None,
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

/// Why [`GalleryRepository::queue_invitations`] could not enqueue invitations.
#[derive(Debug, Error)]
pub enum InvitationError {
    #[error("archived galleries cannot send invitations")]
    Archived,
    #[error("expired galleries cannot send invitations")]
    Expired,
    #[error("the gallery is no longer at the requested stage")]
    StageConflict,
    #[error("a selected gallery contact or address has changed")]
    RecipientChanged,
    #[error(transparent)]
    Database(#[from] DatabaseError),
}

pub use pp_core::types::studio::{FocalCoordinate, FocalPoint};

/// A primary recipient in an [`InvitationBatch`].
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitationContact {
    pub email: Email,
    /// Present for a prefilled gallery contact; an explicit new address has no association yet.
    pub link_id: Option<Uuid>,
    #[serde(default)]
    pub name: Option<ContactName>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InvitationBatchWire {
    stage: Option<InvitationStage>,
    #[serde(default)]
    copy_self: bool,
    #[serde(default)]
    to: Vec<InvitationContact>,
    #[serde(default)]
    bcc: Vec<Email>,
    subject: EmailSubject,
    message: EmailMessage,
}

/// Deduplicated recipients and message body to queue as gallery invitation emails.
///
/// Construction and JSON deserialization deduplicate `to` (keeping the first occurrence) and
/// drop any `bcc` address already present in `to` or earlier in `bcc`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "InvitationBatchWire")]
pub struct InvitationBatch {
    pub stage: InvitationStage,
    pub copy_self: bool,
    to: Vec<InvitationContact>,
    bcc: Vec<Email>,
    pub subject: EmailSubject,
    pub message: EmailMessage,
}

impl InvitationBatch {
    /// Builds a deduplicated invitation batch, keeping the first occurrence in `to` and dropping
    /// any `bcc` address already present in `to` or earlier in `bcc`.
    #[must_use]
    pub fn new(
        mut to: Vec<InvitationContact>,
        mut bcc: Vec<Email>,
        subject: EmailSubject,
        message: EmailMessage,
    ) -> Self {
        let mut seen = HashSet::with_capacity(to.len() + bcc.len());

        to.retain(|contact| seen.insert(contact.email.clone()));
        bcc.retain(|email| seen.insert(email.clone()));

        Self {
            stage: InvitationStage::Proofing,
            copy_self: false,
            to,
            bcc,
            subject,
            message,
        }
    }

    #[must_use]
    pub fn to(&self) -> &[InvitationContact] {
        &self.to
    }

    #[must_use]
    pub fn bcc(&self) -> &[Email] {
        &self.bcc
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.to.is_empty() && self.bcc.is_empty()
    }

    #[must_use]
    pub fn recipient_count(&self) -> usize {
        self.to.len() + self.bcc.len()
    }

    /// Includes a private copy unless this address already has a message in the batch.
    pub fn include_copy(&mut self, email: Email) {
        if !self.to.iter().any(|contact| contact.email == email) && !self.bcc.contains(&email) {
            self.bcc.push(email);
        }
    }

    #[must_use]
    pub fn for_stage(mut self, stage: InvitationStage) -> Self {
        self.stage = stage;
        self
    }
}

impl From<InvitationBatchWire> for InvitationBatch {
    fn from(wire: InvitationBatchWire) -> Self {
        Self {
            stage: wire.stage.unwrap_or(InvitationStage::Proofing),
            copy_self: wire.copy_self,
            ..Self::new(wire.to, wire.bcc, wire.subject, wire.message)
        }
    }
}

/// Outcome of [`GalleryRepository::queue_invitations`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QueuedInvitations {
    pub gallery: Gallery,
    pub queued_count: usize,
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

impl From<GalleryId> for Uuid {
    fn from(id: GalleryId) -> Self {
        id.0
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

/// Arrangement of the client gallery cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "cover_layout", rename_all = "snake_case")]
pub enum CoverLayout {
    #[default]
    FullBleed,
    Split,
    Framed,
    Banner,
}

/// Curated typeface preset for client gallery headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "heading_font", rename_all = "snake_case")]
pub enum HeadingFont {
    #[default]
    Editorial,
    FineArt,
    Timeless,
    Modern,
}

/// Column density preset for the client photo grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "grid_density", rename_all = "snake_case")]
pub enum GridDensity {
    #[default]
    Compact,
    Large,
}

/// Gap spacing preset between photos in the client grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "grid_spacing", rename_all = "snake_case")]
pub enum GridSpacing {
    #[default]
    Compact,
    Generous,
}

/// The result of [`GalleryRepository::deliver`].
#[derive(Debug)]
pub enum Delivery {
    Delivered(Box<Gallery>),
    /// The gallery is not shared yet, or is archived.
    NotDeliverable,
    NotFound,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeliveryIntent {
    Manual,
    Announce,
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
    /// Photographer-only pin that sorts this gallery ahead of unfavorited ones on the dashboard.
    pub favorite: bool,
    pub layout: GalleryLayout,
    pub cover_layout: CoverLayout,
    pub focal_point: FocalPoint,
    pub heading_font: HeadingFont,
    pub grid_density: GridDensity,
    pub grid_spacing: GridSpacing,
    /// `None` means the gallery never expires; when set, the gallery remains accessible through
    /// the end of that UTC calendar day.
    pub expires_at: Option<Date>,
    /// `None` means selection is unlimited.
    pub included_photos: Option<NonZeroUsize>,
    pub watermark_enabled: bool,
    /// Optional 6-digit access code; `None` means the shared link requires no code.
    pub access_code: Option<GalleryAccessCode>,
    /// `None` means the gallery has no hero photo.
    pub hero_id: Option<PhotoId>,
    /// `None` means the gallery has not been shared.
    pub share_token: Option<ShareToken>,
    /// When a client last opened the shared gallery, to within the throttle delay; `None` means
    /// not yet.
    pub last_opened_at: Option<Timestamp>,
    /// The day of the shoot; `None` means none was given.
    pub shoot_date: Option<Date>,
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
            favorite: row.try_get("favorite")?,
            layout: row.try_get("layout")?,
            cover_layout: row.try_get("cover_layout")?,
            focal_point: FocalPoint::from_row(row)?,
            heading_font: row.try_get("heading_font")?,
            grid_density: row.try_get("grid_density")?,
            grid_spacing: row.try_get("grid_spacing")?,
            expires_at: row.try_get("expires_at")?,
            included_photos,
            watermark_enabled: row.try_get("watermark_enabled")?,
            access_code: row.try_get("access_code")?,
            hero_id: row.try_get("hero_id")?,
            share_token: row.try_get("share_token")?,
            last_opened_at: row.try_get("last_opened_at")?,
            shoot_date: row.try_get("shoot_date")?,
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
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
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub favorite: Option<bool>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub layout: Option<GalleryLayout>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub cover_layout: Option<CoverLayout>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub focal_point: Option<FocalPoint>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub heading_font: Option<HeadingFont>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub grid_density: Option<GridDensity>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub grid_spacing: Option<GridSpacing>,
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub expires_at: Option<Option<Date>>,
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub included_photos: Option<Option<NonZeroUsize>>,
    #[serde(
        default,
        deserialize_with = "Present::deserialize",
        skip_serializing_if = "Option::is_none"
    )]
    pub watermark_enabled: Option<bool>,
    /// Optional 6-digit access code (`Some(Some(code))` sets, `Some(None)` clears, `None` keeps);
    /// locked once the gallery has been shared.
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub access_code: Option<Option<GalleryAccessCode>>,
    /// Must be a ready photo of the gallery; anything else leaves the gallery unchanged.
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub hero_id: Option<Option<PhotoId>>,
    /// A calendar day written `YYYY-MM-DD`.
    #[serde(
        default,
        with = "serde_with::rust::double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub shoot_date: Option<Option<Date>>,
}

impl GalleryChanges {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Whether any setting other than `favorite` is being modified, so starring or unstarring
    /// alone leaves `updated_at` untouched.
    const fn touches_updated_at(&self) -> bool {
        self.title.is_some()
            || self.layout.is_some()
            || self.cover_layout.is_some()
            || self.focal_point.is_some()
            || self.heading_font.is_some()
            || self.grid_density.is_some()
            || self.grid_spacing.is_some()
            || self.expires_at.is_some()
            || self.included_photos.is_some()
            || self.watermark_enabled.is_some()
            || self.access_code.is_some()
            || self.hero_id.is_some()
            || self.shoot_date.is_some()
    }
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
            Self::Updated => "favorite DESC, updated_at DESC, id",
            Self::Created => "favorite DESC, created_at DESC, id",
            Self::Oldest => "favorite DESC, created_at, id",
            Self::Title => "favorite DESC, lower(title) NULLS LAST, id",
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
            Self::Expired => "COALESCE(expires_at < (now() AT TIME ZONE 'UTC')::date, false)",
        }
    }
}

/// Data access for galleries; see the module docs.
#[derive(Clone, Copy)]
pub struct GalleryRepository<'db> {
    pool: &'db PgPool,
    options: ActivityOptions,
}

impl<'db> Repository<'db> for GalleryRepository<'db> {
    type Options = ActivityOptions;

    fn open(pool: &'db PgPool, options: Self::Options) -> Self {
        Self { pool, options }
    }
}

impl GalleryRepository<'_> {
    // Interpolated identifiers are fixed here or selected from GallerySort's closed enum.
    const COLUMNS: &'static str = "id,title,favorite,layout,cover_layout,hero_focal_x,hero_focal_y,heading_font,grid_density,grid_spacing,expires_at,included_photos,watermark_enabled,access_code,hero_id,share_token,shared_at,last_opened_at,shoot_date,delivered_at,archived_at,COALESCE(expires_at < (now() AT TIME ZONE 'UTC')::date, false) AS expired,created_at,updated_at";

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
    /// favorite write either lands before delivery or is rejected after it. Delivery cancels
    /// unpublished watermark-only work unless a worker has already locked its job to publish.
    ///
    /// [`PhotoRepository::set_favorite`]: crate::photo::PhotoRepository::set_favorite
    pub async fn deliver(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Delivery, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let delivery =
            Self::deliver_in(&mut transaction, workspace_id, id, DeliveryIntent::Manual).await?;

        if matches!(delivery, Delivery::Delivered(_)) {
            transaction.commit().await?;
        }

        Ok(delivery)
    }

    async fn deliver_in(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: WorkspaceId,
        id: GalleryId,
        intent: DeliveryIntent,
    ) -> Result<Delivery, DatabaseError> {
        // Exclude settings saves and upload/retry admission, not activity's FK check.
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
            .bind(workspace_id)
            .fetch_optional(&mut **transaction)
            .await?;

        let stage: Option<(bool, bool)> = sqlx::query_as(
            "SELECT shared_at IS NOT NULL AND archived_at IS NULL, delivered_at IS NULL
             FROM galleries WHERE id=$1 AND workspace_id=$2",
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?;
        let first_delivery = match stage {
            Some((true, false)) if intent == DeliveryIntent::Announce => {
                return Ok(Delivery::NotDeliverable);
            }
            Some((true, first_delivery)) => first_delivery,
            Some((false, _)) => return Ok(Delivery::NotDeliverable),
            None => return Ok(Delivery::NotFound),
        };

        if first_delivery {
            let photos: Vec<PhotoId> = sqlx::query_scalar(
                "SELECT id FROM photos WHERE workspace_id=$1 AND gallery_id=$2
                   AND watermark_only_pending",
            )
            .bind(workspace_id)
            .bind(id)
            .fetch_all(&mut **transaction)
            .await?;

            if !photos.is_empty() {
                // Like deletion, lock jobs before the gallery and photos so publication may finish.
                JobRepository::delete_photo_jobs_in(transaction, &photos).await?;
            }
        }

        let delivered = sqlx::query_as(AssertSqlSafe(format!(
            "UPDATE galleries SET delivered_at = COALESCE(delivered_at, now())
             WHERE id=$1 AND workspace_id=$2 AND shared_at IS NOT NULL AND archived_at IS NULL
             RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?;

        let Some(gallery) = delivered else {
            // An archive or delete may have won after checking eligibility. Do not lose jobs.
            let exists = Self::exists_in_workspace(&mut **transaction, workspace_id, id).await?;

            return Ok(if exists {
                Delivery::NotDeliverable
            } else {
                Delivery::NotFound
            });
        };

        if first_delivery {
            sqlx::query(
                "UPDATE photos SET status='ready', failure_reason=NULL,
                     requested_watermark_revision=watermark_revision, watermark_only_pending=false
                 WHERE workspace_id=$1 AND gallery_id=$2 AND watermark_only_pending",
            )
            .bind(workspace_id)
            .bind(id)
            .execute(&mut **transaction)
            .await?;
        }

        Ok(Delivery::Delivered(Box::new(gallery)))
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

    async fn assign_share_token<'c, E>(
        executor: E,
        workspace_id: WorkspaceId,
        id: GalleryId,
        candidate: &ShareToken,
    ) -> Result<Option<Gallery>, DatabaseError>
    where
        E: PgExecutor<'c>,
    {
        // The row lock taken by UPDATE makes concurrent calls converge on the first token.
        Ok(sqlx::query_as::<_, Gallery>(AssertSqlSafe(format!(
            "UPDATE galleries SET share_token = COALESCE(share_token, $3),
                 shared_at = COALESCE(shared_at, now())
             WHERE id = $1 AND workspace_id = $2
               AND (share_token IS NOT NULL
                    OR NOT EXISTS (SELECT 1 FROM gallery_contacts WHERE share_token = $3))
             RETURNING {}",
            Self::COLUMNS
        )))
        .bind(id)
        .bind(workspace_id)
        .bind(candidate)
        .fetch_optional(executor)
        .await?)
    }

    async fn exists_in_workspace<'c, E>(
        executor: E,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<bool, DatabaseError>
    where
        E: PgExecutor<'c>,
    {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM galleries WHERE id = $1 AND workspace_id = $2)",
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_one(executor)
        .await?)
    }

    /// Returns the gallery with its private link, creating the link if it has none. A shared
    /// gallery keeps its token for good. Sharing is not an edit, so `updated_at` is untouched.
    pub async fn share(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<Option<Gallery>, InvitationError> {
        self.share_with(workspace_id, id, ShareToken::generate)
            .await
    }

    pub(crate) async fn share_with(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
        next_token: impl FnMut() -> ShareToken,
    ) -> Result<Option<Gallery>, InvitationError> {
        let mut tx = self.pool.begin().await.map_err(DatabaseError::from)?;
        let archived: Option<bool> = sqlx::query_scalar(
            "SELECT archived_at IS NOT NULL FROM galleries WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await.map_err(DatabaseError::from)?;

        match archived {
            None => return Ok(None),
            Some(true) => return Err(InvitationError::Archived),
            Some(false) => (),
        }

        let gallery = Self::share_in(&mut tx, workspace_id, id, next_token).await?;
        tx.commit().await.map_err(DatabaseError::from)?;

        Ok(gallery)
    }

    pub(crate) async fn share_in(
        tx: &mut Transaction<'_, Postgres>,
        workspace_id: WorkspaceId,
        id: GalleryId,
        next_token: impl FnMut() -> ShareToken,
    ) -> Result<Option<Gallery>, DatabaseError> {
        ShareToken::retry_in(
            tx,
            UniqueViolation::GalleryShareToken,
            next_token,
            async |tx, candidate| {
                if let Some(gallery) =
                    Self::assign_share_token(&mut **tx, workspace_id, id, &candidate).await?
                {
                    return Ok(TokenAttempt::Done(Some(gallery)));
                }

                if !Self::exists_in_workspace(&mut **tx, workspace_id, id).await? {
                    return Ok(TokenAttempt::Done(None));
                }

                Ok(TokenAttempt::Collision)
            },
        )
        .await
    }

    /// Shares a draft or delivers a shared gallery, links primary contacts, and queues one email
    /// per recipient in one transaction. A completed stage cannot be announced again.
    pub async fn queue_invitations(
        &self,
        workspace_id: WorkspaceId,
        gallery_id: GalleryId,
        batch: &InvitationBatch,
        job_options: JobOptions,
    ) -> Result<Option<QueuedInvitations>, InvitationError> {
        let mut tx = self.pool.begin().await.map_err(DatabaseError::from)?;
        let stage = batch.stage;
        let gallery = match stage {
            InvitationStage::Proofing => {
                let state: Option<(bool, bool, bool)> = sqlx::query_as(
                    "SELECT archived_at IS NOT NULL,
                            COALESCE(expires_at < (now() AT TIME ZONE 'UTC')::date, false),
                            shared_at IS NOT NULL
                     FROM galleries WHERE id=$1 AND workspace_id=$2 FOR UPDATE",
                )
                .bind(gallery_id)
                .bind(workspace_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(DatabaseError::from)?;

                match state {
                    None => return Ok(None),
                    Some((true, _, _)) => return Err(InvitationError::Archived),
                    Some((_, true, _)) => return Err(InvitationError::Expired),
                    Some((_, _, true)) => return Err(InvitationError::StageConflict),
                    Some((false, false, false)) => (),
                }

                let Some(gallery) =
                    Self::share_in(&mut tx, workspace_id, gallery_id, ShareToken::generate).await?
                else {
                    return Ok(None);
                };

                gallery
            }
            InvitationStage::Finals => {
                match Self::deliver_in(&mut tx, workspace_id, gallery_id, DeliveryIntent::Announce)
                    .await?
                {
                    Delivery::Delivered(gallery)
                        if gallery.states.contains(&GalleryState::Expired) =>
                    {
                        return Err(InvitationError::Expired);
                    }
                    Delivery::Delivered(gallery) => *gallery,
                    Delivery::NotFound => return Ok(None),
                    Delivery::NotDeliverable => return Err(InvitationError::StageConflict),
                }
            }
        };

        for recipient in &batch.to {
            let (contact_id, link_id) = if let Some(link_id) = recipient.link_id {
                let contact_id = sqlx::query_scalar(
                    "SELECT c.id FROM gallery_contacts gc JOIN contacts c ON c.id = gc.contact_id
                     WHERE gc.link_id=$1 AND gc.gallery_id=$2 AND c.workspace_id=$3 AND c.email=$4
                     FOR SHARE OF c, gc",
                )
                .bind(link_id)
                .bind(gallery_id)
                .bind(workspace_id)
                .bind(&recipient.email)
                .fetch_optional(&mut *tx)
                .await
                .map_err(DatabaseError::from)?
                .ok_or(InvitationError::RecipientChanged)?;

                (contact_id, link_id)
            } else {
                let fallback = recipient
                    .name
                    .clone()
                    .unwrap_or_else(|| ContactName::from_email_local_part(&recipient.email));

                ContactRepository::upsert_and_link_in(
                    &mut tx,
                    workspace_id,
                    gallery_id,
                    &recipient.email,
                    &fallback,
                    ShareToken::generate,
                )
                .await?
            };

            JobRepository::enqueue_in(
                &mut tx,
                SendEmail {
                    workspace_id,
                    gallery_id,
                    recipient: EmailRecipient::Contact {
                        contact_id,
                        link_id,
                        email: recipient.email.clone(),
                    },
                    stage,
                    subject: batch.subject.clone(),
                    message: batch.message.clone(),
                },
                job_options.max_attempts,
            )
            .await?;
        }

        for bcc_email in &batch.bcc {
            JobRepository::enqueue_in(
                &mut tx,
                SendEmail {
                    workspace_id,
                    gallery_id,
                    recipient: EmailRecipient::Bcc {
                        email: bcc_email.clone(),
                    },
                    stage,
                    subject: batch.subject.clone(),
                    message: batch.message.clone(),
                },
                job_options.max_attempts,
            )
            .await?;
        }

        tx.commit().await.map_err(DatabaseError::from)?;

        Ok(Some(QueuedInvitations {
            gallery,
            queued_count: batch.recipient_count(),
        }))
    }

    /// Finds the gallery behind a general or per-contact private link, with where its photos
    /// live. Unshared, unknown, archived and expired links are the same `None`, so a client
    /// cannot tell them apart; the database clock decides expiry.
    pub async fn find_shared(
        &self,
        token: &ShareToken,
    ) -> Result<Option<SharedAccess>, DatabaseError> {
        Ok(sqlx::query_as(
            "SELECT g.id AS gallery_id,
                    g.workspace_id,
                    g.title,
                    g.layout,
                    g.cover_layout,
                    g.hero_focal_x,
                    g.hero_focal_y,
                    g.heading_font,
                    g.grid_density,
                    g.grid_spacing,
                    g.shoot_date,
                    g.expires_at,
                    g.included_photos,
                    g.watermark_enabled,
                    g.access_code,
                    g.delivered_at IS NOT NULL AS delivered,
                    (SELECT p.id FROM photos p
                     WHERE p.id = g.hero_id AND p.clean_ready AND p.kind = 'proof'
                       AND p.gallery_id = g.id AND p.workspace_id = g.workspace_id) AS hero_id,
                    matched.contact_id,
                    $1 AS link_token
             FROM (
                 SELECT id AS gallery_id, NULL::uuid AS contact_id
                 FROM galleries
                 WHERE share_token = $1
                 UNION ALL
                 SELECT gallery_id, contact_id
                 FROM gallery_contacts
                 WHERE share_token = $1
             ) AS matched
             JOIN galleries g ON g.id = matched.gallery_id
             WHERE g.shared_at IS NOT NULL
               AND g.archived_at IS NULL
               AND (g.expires_at IS NULL OR g.expires_at >= (now() AT TIME ZONE 'UTC')::date)
             ORDER BY matched.contact_id NULLS FIRST
             LIMIT 1",
        )
        .bind(token)
        .fetch_optional(self.pool)
        .await?)
    }

    /// Checks the claim, stage, link and destination of a prepared invitation just before sending.
    pub async fn invitation_is_current(
        &self,
        job: &Job,
        location: PhotoLocation,
        stage: InvitationStage,
        token: &ShareToken,
        email: &Email,
    ) -> Result<bool, DatabaseError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (SELECT FROM galleries g
             WHERE g.id=$1 AND g.workspace_id=$2 AND g.archived_at IS NULL AND g.shared_at IS NOT NULL
               AND EXISTS (SELECT FROM jobs WHERE id=$6 AND claim_generation=$7 AND state='running')
               AND (g.expires_at IS NULL OR g.expires_at >= (now() AT TIME ZONE 'UTC')::date)
               AND (g.delivered_at IS NOT NULL) = $3
               AND (g.share_token=$4 OR EXISTS (SELECT FROM gallery_contacts gc
                    JOIN contacts c ON c.id=gc.contact_id AND c.workspace_id=g.workspace_id
                    WHERE gc.gallery_id=g.id AND gc.share_token=$4 AND c.email=$5)))",
        )
        .bind(location.gallery_id)
        .bind(location.workspace_id)
        .bind(stage == InvitationStage::Finals)
        .bind(token)
        .bind(email)
        .bind(job.id)
        .bind(job.claim_generation)
        .fetch_one(self.pool)
        .await?)
    }

    /// Records a client open of a gallery reached through its private link. A gallery opened less
    /// than [`Self::OPEN_THROTTLE`] ago keeps its time, which bounds the writes of reloads and
    /// repeat visits at the price of precision. An archived or expired gallery is not marked. Returns whether this
    /// call recorded the open.
    pub async fn record_opened(&self, location: PhotoLocation) -> Result<bool, DatabaseError> {
        let mut transaction = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE galleries SET last_opened_at = now()
             WHERE id=$1 AND workspace_id=$2 AND archived_at IS NULL
               AND (expires_at IS NULL OR expires_at >= (now() AT TIME ZONE 'UTC')::date)
               AND (last_opened_at IS NULL OR last_opened_at < now() - make_interval(secs => $3))",
        )
        .bind(location.gallery_id)
        .bind(location.workspace_id)
        .bind(Self::OPEN_THROTTLE.as_secs_f64())
        .execute(&mut *transaction)
        .await?;
        let opened = result.rows_affected() == 1;

        if opened {
            transaction
                .record_activity(&self.options, location, ActivityKind::Open, &[], None)
                .await?;
        }

        transaction.commit().await?;

        Ok(opened)
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

        let updated = if changes.hero_id.flatten().is_some() {
            // The hero check locks a proof. Take the gallery first, like proof deletion.
            let mut transaction = self.pool.begin().await.map_err(DatabaseError::from)?;
            Self::lock_for_update_in(&mut transaction, workspace_id, id).await?;
            let updated = Self::update_in(&mut *transaction, workspace_id, id, changes).await?;
            transaction.commit().await.map_err(DatabaseError::from)?;

            updated
        } else {
            Self::update_in(self.pool, workspace_id, id, changes).await?
        };

        if updated.is_none()
            && changes.access_code.is_some()
            && self.get(workspace_id, id).await?.is_some_and(|gallery| {
                gallery.share_token.is_some() || !gallery.states.contains(&GalleryState::Draft)
            })
        {
            return Err(GalleryError::AccessCodeLocked);
        }

        Ok(updated)
    }

    /// Locks this gallery for a hero change before the transaction takes proof locks.
    pub(crate) async fn lock_for_update_in(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: WorkspaceId,
        id: GalleryId,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "SELECT id FROM galleries WHERE id = $1 AND workspace_id = $2 FOR NO KEY UPDATE",
        )
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(&mut **transaction)
        .await?;

        Ok(())
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
                shoot_date = CASE WHEN $13 THEN $14 ELSE shoot_date END,
                cover_layout = COALESCE($15, cover_layout),
                hero_focal_x = COALESCE($16, hero_focal_x),
                hero_focal_y = COALESCE($17, hero_focal_y),
                heading_font = COALESCE($18, heading_font),
                grid_density = COALESCE($19, grid_density),
                grid_spacing = COALESCE($20, grid_spacing),
                access_code = CASE WHEN $21 THEN $22 ELSE access_code END,
                favorite = COALESCE($23, favorite),
                updated_at = CASE WHEN $24 THEN now() ELSE updated_at END
             WHERE id=$1 AND workspace_id=$2
               AND (NOT $21 OR (shared_at IS NULL AND share_token IS NULL))
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
        .bind(changes.shoot_date.is_some())
        .bind(changes.shoot_date.flatten())
        .bind(changes.cover_layout)
        .bind(changes.focal_point.map(|point| point.x))
        .bind(changes.focal_point.map(|point| point.y))
        .bind(changes.heading_font)
        .bind(changes.grid_density)
        .bind(changes.grid_spacing)
        .bind(changes.access_code.is_some())
        .bind(changes.access_code.as_ref().and_then(Option::as_ref))
        .bind(changes.favorite)
        .bind(changes.touches_updated_at())
        .fetch_optional(executor)
        .await
        .map_err(GalleryError::classify)
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, time::SystemTime};

    use serde_json::json;

    use super::*;
    use crate::{
        Database,
        activity::Client,
        auth::AuthRepository,
        contact::NewContact,
        jobs::{JobKind, JobOptions, JobPayload, JobRepository, ProcessPhoto},
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

        set_column(
            &pool,
            gallery.id,
            "expires_at",
            "(now() AT TIME ZONE 'UTC')::date",
        )
        .await;
        let today = store.get(workspace, gallery.id).await.unwrap().unwrap();

        assert_eq!(today.states, [GalleryState::Draft]);

        set_column(
            &pool,
            gallery.id,
            "expires_at",
            "(now() AT TIME ZONE 'UTC')::date - 1",
        )
        .await;
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
        assert_eq!(gallery.cover_layout, CoverLayout::FullBleed);
        assert_eq!(gallery.focal_point, FocalPoint::default());
        assert_eq!(gallery.heading_font, HeadingFont::Editorial);
        assert_eq!(gallery.grid_density, GridDensity::Compact);
        assert_eq!(gallery.grid_spacing, GridSpacing::Compact);
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
                    "expires_at": "2030-01-02",
                    "included_photos": 25,
                    "watermark_enabled": true,
                })),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(set.title.as_deref(), Some("Wedding"));
        assert_eq!(set.layout, GalleryLayout::Masonry);
        assert_eq!(
            set.expires_at,
            Some(Date::from_calendar_date(2030, time::Month::January, 2).unwrap())
        );
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
            json!({"cover_layout": null}),
            json!({"cover_layout": "carousel"}),
            json!({"heading_font": null}),
            json!({"heading_font": "comic_sans"}),
            json!({"grid_density": null}),
            json!({"grid_density": "medium"}),
            json!({"grid_spacing": null}),
            json!({"grid_spacing": "wide"}),
            json!({"focal_point": null}),
            json!({"focal_point": {"x": 101, "y": 35}}),
            json!({"focal_point": {"x": -1, "y": 35}}),
            json!({"focal_point": {"x": 50, "y": 101}}),
            json!({"focal_point": {"x": 50, "y": -1}}),
            json!({"focal_point": {"x": 50}}),
            json!({"focal_point": {"x": 50, "y": 35, "z": 0}}),
            json!({"watermark_enabled": null}),
            json!({"access_code": ""}),
            json!({"access_code": "12345"}),
            json!({"access_code": "1234567"}),
            json!({"access_code": "abcdef"}),
            json!({"access_code": 123456}),
            json!({"included_photos": 0}),
            json!({"layout": "carousel"}),
            json!({"colour": "red"}),
            json!({"expires_at": "2030-01-02T03:04:05Z"}),
            json!({"expires_at": "9999-12-31T23:59:59-01:00"}),
            json!({"expires_at": "02/01/2030"}),
            json!({"expires_at": "2030-02-30"}),
        ] {
            assert!(
                serde_json::from_value::<GalleryChanges>(body.clone()).is_err(),
                "{body}"
            );
        }

        assert!(changes(json!({})).is_empty());
        assert!(!changes(json!({"included_photos": null})).is_empty());
        assert!(!changes(json!({"title": null})).is_empty());

        let partial = serde_json::from_value::<GalleryChanges>(json!({
            "title": null,
            "watermark_enabled": true
        }))
        .unwrap();

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
        set_column(
            &pool,
            ids[4],
            "expires_at",
            "(now() AT TIME ZONE 'UTC')::date - 1",
        )
        .await;

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
    async fn archived_draft_is_not_shared_manually(pool: PgPool) {
        let db = Database::new(pool.clone());
        let galleries = db.repository::<GalleryRepository>();
        let owner = workspace(&pool).await;
        let id = galleries.create(owner).await.unwrap().id;
        galleries.archive(owner, id).await.unwrap();

        assert!(galleries.share(owner, id).await.is_err());
        assert!(
            galleries
                .get(owner, id)
                .await
                .unwrap()
                .unwrap()
                .share_token
                .is_none()
        );
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

    /// The gallery's activity lines, as `(kind, count)`, oldest first.
    async fn activity_lines(pool: &PgPool, gallery_id: GalleryId) -> Vec<(ActivityKind, i32)> {
        sqlx::query_as(
            "SELECT kind, count FROM activity_events
             WHERE gallery_id = $1 ORDER BY first_at, id",
        )
        .bind(gallery_id)
        .fetch_all(pool)
        .await
        .unwrap()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn record_opened_records_only_when_it_actually_opens(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace = workspace(&pool).await;
        let client = Some(Client { contact_id: None });
        let store = database
            .repository_with::<GalleryRepository>(ActivityOptions::default().client(client));
        let gallery = store.create(workspace).await.unwrap();
        let token = store
            .share(workspace, gallery.id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let location = store.find_shared(&token).await.unwrap().unwrap().location;

        assert!(store.record_opened(location).await.unwrap());
        // Still inside the throttle: no new open, so no new event either.
        assert!(!store.record_opened(location).await.unwrap());

        assert_eq!(
            activity_lines(&pool, gallery.id).await,
            [(ActivityKind::Open, 1)]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn record_opened_without_a_client_records_nothing(pool: PgPool) {
        let database = Database::new(pool.clone());
        let workspace = workspace(&pool).await;
        let store = database.repository::<GalleryRepository>();
        let gallery = store.create(workspace).await.unwrap();
        let token = store
            .share(workspace, gallery.id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let location = store.find_shared(&token).await.unwrap().unwrap().location;

        assert!(store.record_opened(location).await.unwrap());

        assert!(activity_lines(&pool, gallery.id).await.is_empty());
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
                &changes(json!({"expires_at": "2000-01-01"})),
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

        let notifications: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(notifications, 0, "an unavailable link records no open");
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

        set_expiry(json!("2000-01-01")).await;

        assert_eq!(store.find_shared(&token).await.unwrap(), None);

        let today_utc: Date = sqlx::query_scalar("SELECT (now() AT TIME ZONE 'UTC')::date")
            .fetch_one(&pool)
            .await
            .unwrap();
        set_expiry(serde_json::to_value(today_utc).unwrap()).await;

        assert_eq!(
            store
                .find_shared(&token)
                .await
                .unwrap()
                .unwrap()
                .gallery
                .expires_at,
            Some(today_utc)
        );

        set_expiry(json!("2999-01-01")).await;

        assert_eq!(
            store
                .find_shared(&token)
                .await
                .unwrap()
                .unwrap()
                .gallery
                .expires_at,
            Some(Date::from_calendar_date(2999, time::Month::January, 1).unwrap())
        );

        set_expiry(json!("2000-01-01")).await;
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
        set_column(
            &pool,
            id,
            "expires_at",
            "(now() AT TIME ZONE 'UTC')::date - 1",
        )
        .await;
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

    #[test]
    fn cover_layout_and_focal_point_serialize_and_enforce_bounds() {
        assert_eq!(CoverLayout::default(), CoverLayout::FullBleed);

        for (layout, expected) in [
            (CoverLayout::FullBleed, json!("full_bleed")),
            (CoverLayout::Split, json!("split")),
            (CoverLayout::Framed, json!("framed")),
            (CoverLayout::Banner, json!("banner")),
        ] {
            assert_eq!(serde_json::to_value(layout).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<CoverLayout>(expected).unwrap(),
                layout
            );
        }

        let default_point = FocalPoint::default();

        assert_eq!((default_point.x.get(), default_point.y.get()), (50, 35));
        assert_eq!(
            serde_json::to_value(default_point).unwrap(),
            json!({"x": 50, "y": 35})
        );

        let corners: FocalPoint = serde_json::from_value(json!({"x": 0, "y": 100})).unwrap();

        assert_eq!((corners.x.get(), corners.y.get()), (0, 100));
        assert!(FocalCoordinate::try_from(0).is_ok());
        assert!(FocalCoordinate::try_from(100).is_ok());
        assert!(FocalCoordinate::try_from(101).is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_adds_cover_layout_and_bounded_focal_point_defaults(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("cover_layout_migration_{}", Uuid::new_v4().simple());

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
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace_id = Uuid::new_v4();
        let gallery_id = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
            .bind(gallery_id)
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();

        sqlx::raw_sql(include_str!(
            "../migrations/0030_cover_layout_and_focal_point.sql"
        ))
        .execute(&mut *transaction)
        .await
        .unwrap();

        let row: (String, i16, i16) = sqlx::query_as(
            "SELECT cover_layout::text, hero_focal_x, hero_focal_y FROM galleries WHERE id = $1",
        )
        .bind(gallery_id)
        .fetch_one(&mut *transaction)
        .await
        .unwrap();

        assert_eq!(row, ("full_bleed".to_owned(), 50, 35));

        for statement in [
            "UPDATE galleries SET hero_focal_x = -1",
            "UPDATE galleries SET hero_focal_x = 101",
            "UPDATE galleries SET hero_focal_y = -1",
            "UPDATE galleries SET hero_focal_y = 101",
            "UPDATE galleries SET cover_layout = 'carousel'",
        ] {
            sqlx::raw_sql("SAVEPOINT invalid_cover_write")
                .execute(&mut *transaction)
                .await
                .unwrap();

            let outcome = sqlx::raw_sql(statement).execute(&mut *transaction).await;

            assert!(outcome.is_err(), "{statement}");

            sqlx::raw_sql("ROLLBACK TO SAVEPOINT invalid_cover_write")
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn cover_layout_focal_point_and_shoot_date_reach_owner_and_shared_views(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;
        let updated = store
            .update(
                workspace,
                id,
                &changes(json!({
                    "cover_layout": "framed",
                    "focal_point": {"x": 22, "y": 78},
                    "shoot_date": "2026-09-19",
                })),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.cover_layout, CoverLayout::Framed);
        assert_eq!(
            (updated.focal_point.x.get(), updated.focal_point.y.get()),
            (22, 78)
        );
        assert!(updated.shoot_date.is_some());

        let token = store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let shared = store.find_shared(&token).await.unwrap().unwrap().gallery;

        assert_eq!(shared.cover_layout, CoverLayout::Framed);
        assert_eq!(shared.focal_point, updated.focal_point);
        assert_eq!(shared.shoot_date, updated.shoot_date);
    }

    #[test]
    fn typography_and_grid_presets_serialize_and_default() {
        assert_eq!(HeadingFont::default(), HeadingFont::Editorial);
        assert_eq!(GridDensity::default(), GridDensity::Compact);
        assert_eq!(GridSpacing::default(), GridSpacing::Compact);

        for (font, expected) in [
            (HeadingFont::Editorial, json!("editorial")),
            (HeadingFont::FineArt, json!("fine_art")),
            (HeadingFont::Timeless, json!("timeless")),
            (HeadingFont::Modern, json!("modern")),
        ] {
            assert_eq!(serde_json::to_value(font).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<HeadingFont>(expected).unwrap(),
                font
            );
        }

        for (density, expected) in [
            (GridDensity::Compact, json!("compact")),
            (GridDensity::Large, json!("large")),
        ] {
            assert_eq!(serde_json::to_value(density).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<GridDensity>(expected).unwrap(),
                density
            );
        }

        for (spacing, expected) in [
            (GridSpacing::Compact, json!("compact")),
            (GridSpacing::Generous, json!("generous")),
        ] {
            assert_eq!(serde_json::to_value(spacing).unwrap(), expected);
            assert_eq!(
                serde_json::from_value::<GridSpacing>(expected).unwrap(),
                spacing
            );
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_adds_typography_and_grid_preset_defaults(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("typography_grid_migration_{}", Uuid::new_v4().simple());

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
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace_id = Uuid::new_v4();
        let gallery_id = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
            .bind(gallery_id)
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();

        sqlx::raw_sql(include_str!(
            "../migrations/0032_gallery_typography_and_grid_presets.sql"
        ))
        .execute(&mut *transaction)
        .await
        .unwrap();

        let row: (String, String, String) = sqlx::query_as(
            "SELECT heading_font::text, grid_density::text, grid_spacing::text
             FROM galleries WHERE id = $1",
        )
        .bind(gallery_id)
        .fetch_one(&mut *transaction)
        .await
        .unwrap();

        assert_eq!(
            row,
            (
                "editorial".to_owned(),
                "compact".to_owned(),
                "compact".to_owned()
            )
        );

        for statement in [
            "UPDATE galleries SET heading_font = 'comic_sans'",
            "UPDATE galleries SET grid_density = 'medium'",
            "UPDATE galleries SET grid_spacing = 'wide'",
        ] {
            sqlx::raw_sql("SAVEPOINT invalid_preset_write")
                .execute(&mut *transaction)
                .await
                .unwrap();

            let outcome = sqlx::raw_sql(statement).execute(&mut *transaction).await;

            assert!(outcome.is_err(), "{statement}");

            sqlx::raw_sql("ROLLBACK TO SAVEPOINT invalid_preset_write")
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn typography_and_grid_presets_reach_owner_and_shared_views(pool: PgPool) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let id = store.create(workspace).await.unwrap().id;
        let updated = store
            .update(
                workspace,
                id,
                &changes(json!({
                    "heading_font": "fine_art",
                    "grid_density": "large",
                    "grid_spacing": "generous",
                })),
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(updated.heading_font, HeadingFont::FineArt);
        assert_eq!(updated.grid_density, GridDensity::Large);
        assert_eq!(updated.grid_spacing, GridSpacing::Generous);

        let kept = store
            .update(workspace, id, &changes(json!({"title": "Portraits"})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(kept.heading_font, HeadingFont::FineArt);
        assert_eq!(kept.grid_density, GridDensity::Large);
        assert_eq!(kept.grid_spacing, GridSpacing::Generous);

        let token = store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let shared = store.find_shared(&token).await.unwrap().unwrap().gallery;

        assert_eq!(shared.heading_font, HeadingFont::FineArt);
        assert_eq!(shared.grid_density, GridDensity::Large);
        assert_eq!(shared.grid_spacing, GridSpacing::Generous);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_replaces_access_code_ciphertext_with_plain_six_digit_code(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("access_code_migration_{}", Uuid::new_v4().simple());

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
            include_str!("../migrations/0033_gallery_access_code.sql"),
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace_id = Uuid::new_v4();
        let unlocked_id = Uuid::new_v4();
        let locked_a = Uuid::new_v4();
        let locked_b = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();

        for id in [unlocked_id, locked_a, locked_b] {
            sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
                .bind(id)
                .bind(workspace_id)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        for (id, byte) in [(locked_a, 0x11u8), (locked_b, 0x22u8)] {
            sqlx::query("UPDATE galleries SET access_code_ciphertext = $1 WHERE id = $2")
                .bind(vec![byte; 34])
                .bind(id)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        sqlx::raw_sql(include_str!(
            "../migrations/0035_plain_gallery_access_code.sql"
        ))
        .execute(&mut *transaction)
        .await
        .unwrap();

        let legacy_column_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM information_schema.columns
                 WHERE table_schema = $1
                   AND table_name = 'galleries'
                   AND column_name = 'access_code_ciphertext'
             )",
        )
        .bind(&schema)
        .fetch_one(&mut *transaction)
        .await
        .unwrap();

        assert!(!legacy_column_exists);

        let unlocked_code: Option<String> =
            sqlx::query_scalar("SELECT access_code FROM galleries WHERE id = $1")
                .bind(unlocked_id)
                .fetch_one(&mut *transaction)
                .await
                .unwrap();

        assert_eq!(unlocked_code, None);

        for locked_id in [locked_a, locked_b] {
            let migrated: Option<GalleryAccessCode> =
                sqlx::query_scalar("SELECT access_code FROM galleries WHERE id = $1")
                    .bind(locked_id)
                    .fetch_one(&mut *transaction)
                    .await
                    .unwrap();

            assert!(
                migrated.is_some(),
                "locked gallery {locked_id} must remain locked with a valid 6-digit code"
            );
        }

        for invalid in ["", "12345", "1234567", "12345a", " 12345", "١٢٣٤٥٦"] {
            sqlx::raw_sql("SAVEPOINT invalid_access_code")
                .execute(&mut *transaction)
                .await
                .unwrap();

            let outcome = sqlx::query("UPDATE galleries SET access_code = $1 WHERE id = $2")
                .bind(invalid)
                .bind(unlocked_id)
                .execute(&mut *transaction)
                .await;

            assert!(outcome.is_err(), "{invalid:?} must be rejected");

            sqlx::raw_sql("ROLLBACK TO SAVEPOINT invalid_access_code")
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        sqlx::query("UPDATE galleries SET access_code = $1 WHERE id = $2")
            .bind("042819")
            .bind(unlocked_id)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("UPDATE galleries SET access_code = NULL WHERE id = $1")
            .bind(unlocked_id)
            .execute(&mut *transaction)
            .await
            .unwrap();

        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_0036_converts_timestamptz_expires_at_to_utc_date(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("expires_at_date_migration_{}", Uuid::new_v4().simple());

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
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace_id = Uuid::new_v4();
        let never_id = Uuid::new_v4();
        let late_evening_id = Uuid::new_v4();
        let next_day_utc_id = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
            .bind(never_id)
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO galleries(id, workspace_id, expires_at)
             VALUES ($1, $2, '2026-10-31T22:59:59Z'::timestamptz)",
        )
        .bind(late_evening_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO galleries(id, workspace_id, expires_at)
             VALUES ($1, $2, '2026-10-31T23:30:00-02:00'::timestamptz)",
        )
        .bind(next_day_utc_id)
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await
        .unwrap();

        sqlx::raw_sql(include_str!(
            "../migrations/0036_gallery_expires_at_date.sql"
        ))
        .execute(&mut *transaction)
        .await
        .unwrap();

        let data_type: String = sqlx::query_scalar(
            "SELECT data_type FROM information_schema.columns
             WHERE table_schema = $1 AND table_name = 'galleries' AND column_name = 'expires_at'",
        )
        .bind(&schema)
        .fetch_one(&mut *transaction)
        .await
        .unwrap();

        assert_eq!(data_type, "date");

        let never_date: Option<Date> =
            sqlx::query_scalar("SELECT expires_at FROM galleries WHERE id = $1")
                .bind(never_id)
                .fetch_one(&mut *transaction)
                .await
                .unwrap();
        let oct_31: Option<Date> =
            sqlx::query_scalar("SELECT expires_at FROM galleries WHERE id = $1")
                .bind(late_evening_id)
                .fetch_one(&mut *transaction)
                .await
                .unwrap();
        let nov_1: Option<Date> =
            sqlx::query_scalar("SELECT expires_at FROM galleries WHERE id = $1")
                .bind(next_day_utc_id)
                .fetch_one(&mut *transaction)
                .await
                .unwrap();

        assert_eq!(never_date, None);
        assert_eq!(
            oct_31,
            Some(Date::from_calendar_date(2026, time::Month::October, 31).unwrap())
        );
        assert_eq!(
            nov_1,
            Some(Date::from_calendar_date(2026, time::Month::November, 1).unwrap())
        );

        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn access_code_edits_lock_once_shared_and_never_serialize_in_overview_or_shared_views(
        pool: PgPool,
    ) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);
        let created = store.create(mine).await.unwrap();
        let first_code: GalleryAccessCode = "042819".parse().unwrap();
        let second_code: GalleryAccessCode = "901234".parse().unwrap();

        assert_eq!(created.access_code, None);
        assert_eq!(format!("{first_code:?}"), "GalleryAccessCode(..)");

        let protected = store
            .update(
                mine,
                created.id,
                &GalleryChanges {
                    access_code: Some(Some(first_code.clone())),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(protected.access_code.as_ref(), Some(&first_code));

        let overview_json =
            serde_json::to_value(&store.list(mine, GallerySort::Updated, &[]).await.unwrap()[0])
                .unwrap();

        assert_eq!(overview_json["access_code"], "042819");
        assert!(overview_json.get("access_code_ciphertext").is_none());
        assert!(overview_json.get("access_code_protected").is_none());

        let cleared = store
            .update(
                mine,
                created.id,
                &GalleryChanges {
                    access_code: Some(None),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap()
            .unwrap();

        assert_eq!(cleared.access_code, None);

        store
            .update(
                mine,
                created.id,
                &GalleryChanges {
                    access_code: Some(Some(first_code.clone())),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap()
            .unwrap();

        let shared = store.share(mine, created.id).await.unwrap().unwrap();
        assert_eq!(shared.access_code.as_ref(), Some(&first_code));
        let token = shared.share_token.unwrap();

        for forbidden in [Some(second_code.clone()), None] {
            let outcome = store
                .update(
                    mine,
                    created.id,
                    &GalleryChanges {
                        access_code: Some(forbidden),
                        ..GalleryChanges::default()
                    },
                )
                .await;

            assert!(matches!(outcome, Err(GalleryError::AccessCodeLocked)));
        }

        // Foreign or missing galleries still return Ok(None) rather than leaking AccessCodeLocked.
        assert_eq!(
            store
                .update(
                    other,
                    created.id,
                    &GalleryChanges {
                        access_code: Some(Some(second_code)),
                        ..GalleryChanges::default()
                    },
                )
                .await
                .unwrap(),
            None
        );

        // Non-access-code settings remain editable after sharing without touching the stored code.
        let retitled = store
            .update(mine, created.id, &changes(json!({"title": "Locked Shoot"})))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(retitled.title.as_deref(), Some("Locked Shoot"));
        assert_eq!(retitled.access_code.as_ref(), Some(&first_code));

        let found = store.find_shared(&token).await.unwrap().unwrap();
        let shared_json = serde_json::to_value(&found.gallery).unwrap();

        assert_eq!(found.access_code.as_ref(), Some(&first_code));
        assert!(shared_json.get("access_code_ciphertext").is_none());
        assert!(shared_json.get("access_code").is_none());

        let contact_token = ShareToken::generate();
        let contact_id = Uuid::new_v4();

        sqlx::query("INSERT INTO contacts(id, workspace_id, name, email) VALUES ($1, $2, 'Claire', 'claire@example.test')")
            .bind(contact_id)
            .bind(mine)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO gallery_contacts(gallery_id, contact_id, share_token) VALUES ($1, $2, $3)",
        )
        .bind(created.id)
        .bind(contact_id)
        .bind(&contact_token)
        .execute(&pool)
        .await
        .unwrap();

        let contact_found = store.find_shared(&contact_token).await.unwrap().unwrap();
        assert_eq!(contact_found.access_code.as_ref(), Some(&first_code));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn migration_adds_favorite_false_default(pool: PgPool) {
        let mut transaction = pool.begin().await.unwrap();
        let schema = format!("favorite_migration_{}", Uuid::new_v4().simple());

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
        ] {
            sqlx::raw_sql(migration)
                .execute(&mut *transaction)
                .await
                .unwrap();
        }

        let workspace_id = Uuid::new_v4();
        let gallery_id = Uuid::new_v4();

        sqlx::query("INSERT INTO workspaces(id) VALUES ($1)")
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::query("INSERT INTO galleries(id, workspace_id) VALUES ($1, $2)")
            .bind(gallery_id)
            .bind(workspace_id)
            .execute(&mut *transaction)
            .await
            .unwrap();

        sqlx::raw_sql(include_str!("../migrations/0034_gallery_favorite.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();

        let favorite: bool = sqlx::query_scalar("SELECT favorite FROM galleries WHERE id = $1")
            .bind(gallery_id)
            .fetch_one(&mut *transaction)
            .await
            .unwrap();

        assert!(!favorite);
        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn favorites_sort_first_across_all_modes_and_keep_updated_at_when_toggled_alone(
        pool: PgPool,
    ) {
        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let workspace = workspace(&pool).await;
        let first = store.create(workspace).await.unwrap();
        let second = store.create(workspace).await.unwrap();

        sqlx::query(
            "UPDATE galleries SET title = 'Beta',
                 created_at = '2026-01-01T10:00:00Z',
                 updated_at = '2026-01-01T10:00:00Z'
             WHERE id = $1",
        )
        .bind(first.id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE galleries SET title = 'Alpha',
                 created_at = '2026-01-02T10:00:00Z',
                 updated_at = '2026-01-02T10:00:00Z'
             WHERE id = $1",
        )
        .bind(second.id)
        .execute(&pool)
        .await
        .unwrap();

        let before = store.get(workspace, first.id).await.unwrap().unwrap();
        assert!(!before.favorite);

        let starred = store
            .update(workspace, first.id, &changes(json!({"favorite": true})))
            .await
            .unwrap()
            .unwrap();

        assert!(starred.favorite);
        assert_eq!(starred.updated_at, before.updated_at);

        for sort in [
            GallerySort::Updated,
            GallerySort::Created,
            GallerySort::Oldest,
            GallerySort::Title,
        ] {
            let ids: Vec<GalleryId> = store
                .list(workspace, sort, &[])
                .await
                .unwrap()
                .into_iter()
                .map(|item| item.gallery.id)
                .collect();

            assert_eq!(ids, vec![first.id, second.id], "{sort:?}");
        }

        let token = store
            .share(workspace, first.id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();
        let shared_json =
            serde_json::to_value(store.find_shared(&token).await.unwrap().unwrap().gallery)
                .unwrap();

        assert!(shared_json.get("favorite").is_none());

        let unstarred = store
            .update(workspace, first.id, &changes(json!({"favorite": false})))
            .await
            .unwrap()
            .unwrap();

        assert!(!unstarred.favorite);
        assert_eq!(unstarred.updated_at, before.updated_at);

        let updated_ids: Vec<GalleryId> = store
            .list(workspace, GallerySort::Updated, &[])
            .await
            .unwrap()
            .into_iter()
            .map(|item| item.gallery.id)
            .collect();

        assert_eq!(updated_ids, vec![second.id, first.id]);
        assert!(serde_json::from_value::<GalleryChanges>(json!({"favorite": null})).is_err());
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn queue_invitations_shares_draft_upserts_contacts_deduplicates_and_guards_states(
        pool: PgPool,
    ) {
        let database = Database::new(pool.clone());
        let galleries = database.repository::<GalleryRepository>();
        let contacts = database.repository::<ContactRepository>();
        let jobs = database.repository::<JobRepository>();
        let auth = database.repository::<AuthRepository>();

        let owner_email: Email = "photographer@example.test".parse().unwrap();
        let identity = auth
            .create_account(
                &owner_email,
                "hash",
                &[0x11; 32],
                SystemTime::now() + Duration::from_secs(3600),
            )
            .await
            .unwrap()
            .unwrap();
        let workspace = identity.workspace_id;
        let other_workspace = super::tests::workspace(&pool).await;

        assert_eq!(
            auth.account_email(workspace).await.unwrap(),
            Some(owner_email)
        );
        assert_eq!(auth.account_email(other_workspace).await.unwrap(), None);

        // Existing workspace contact whose rich name must be preserved even if omitted in `to`.
        let existing_contact = contacts
            .create(
                workspace,
                &NewContact {
                    name: "Camille & Hugo".parse().unwrap(),
                    email: "camille@example.test".parse().unwrap(),
                    phone: None,
                    notes: None,
                },
            )
            .await
            .unwrap();

        let draft = galleries.create(workspace).await.unwrap();
        assert!(draft.share_token.is_none());

        let batch = InvitationBatch::new(
            vec![
                InvitationContact {
                    email: "Camille@Example.test".parse().unwrap(),
                    link_id: None,
                    name: None,
                },
                InvitationContact {
                    email: "claire.dupont@example.test".parse().unwrap(),
                    link_id: None,
                    name: None,
                },
                InvitationContact {
                    email: "camille@example.test".parse().unwrap(),
                    link_id: None,
                    name: Some("Ignored Duplicate".parse().unwrap()),
                },
            ],
            vec![
                "CAMILLE@example.test".parse().unwrap(),
                "planner@example.test".parse().unwrap(),
                "Planner@Example.test".parse().unwrap(),
            ],
            "Camille & Hugo — Your gallery is ready".parse().unwrap(),
            "Bonjour,\n\nVos photos vous attendent.".parse().unwrap(),
        );

        // Foreign workspace sees `Ok(None)`.
        assert!(
            galleries
                .queue_invitations(other_workspace, draft.id, &batch, JobOptions::default())
                .await
                .unwrap()
                .is_none()
        );

        let queued = galleries
            .queue_invitations(workspace, draft.id, &batch, JobOptions::default())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(queued.queued_count, 3);
        assert!(queued.gallery.share_token.is_some());
        assert!(queued.gallery.states.contains(&GalleryState::Shared));

        // Existing contact kept its name; new contact derived local-part; BCC did not create a contact!
        let all_contacts = contacts.list(workspace).await.unwrap();
        assert_eq!(all_contacts.len(), 2);

        let camille = all_contacts
            .iter()
            .find(|c| c.id == existing_contact.id)
            .unwrap();
        assert_eq!(camille.name.as_str(), "Camille & Hugo");

        let claire = all_contacts
            .iter()
            .find(|c| c.email.as_str() == "claire.dupont@example.test")
            .unwrap();
        assert_eq!(claire.name.as_str(), "claire.dupont");

        let linked = contacts
            .list_for_gallery(workspace, draft.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(linked.len(), 2);
        assert!(
            contacts
                .find_gallery_link(workspace, draft.id, claire.id)
                .await
                .unwrap()
                .is_some()
        );

        // Claim the 3 jobs and confirm zero secrets in raw JSON payloads and exact stage/recipients.
        let raw_payloads: Vec<String> =
            sqlx::query_scalar("SELECT payload::text FROM jobs ORDER BY created_at, id")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(raw_payloads.len(), 3);
        let gallery_token_str: String =
            sqlx::query_scalar("SELECT share_token FROM galleries WHERE id = $1")
                .bind(draft.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let contact_tokens: Vec<String> =
            sqlx::query_scalar("SELECT share_token FROM gallery_contacts WHERE gallery_id = $1")
                .bind(draft.id)
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(contact_tokens.len(), 2);

        for raw in &raw_payloads {
            assert!(!raw.contains(&gallery_token_str));

            for contact_token in &contact_tokens {
                assert!(!raw.contains(contact_token));
            }
        }

        let mut recipients = Vec::new();

        while let Some(job) = jobs
            .claim(&[JobKind::SendEmail], Duration::from_secs(30))
            .await
            .unwrap()
        {
            let send: SendEmail = job.payload().unwrap();
            assert_eq!(send.stage, InvitationStage::Proofing);
            recipients.push(send.recipient);
            assert!(jobs.complete(&job).await.unwrap().is_current());
        }

        assert_eq!(
            recipients,
            vec![
                EmailRecipient::Contact {
                    contact_id: existing_contact.id,
                    email: existing_contact.email.clone(),
                    link_id: linked
                        .iter()
                        .find(|c| c.contact.id == existing_contact.id)
                        .unwrap()
                        .link_id,
                },
                EmailRecipient::Contact {
                    contact_id: claire.id,
                    email: claire.email.clone(),
                    link_id: linked
                        .iter()
                        .find(|c| c.contact.id == claire.id)
                        .unwrap()
                        .link_id,
                },
                EmailRecipient::Bcc {
                    email: "planner@example.test".parse().unwrap()
                },
            ]
        );

        let finals = InvitationBatch {
            stage: InvitationStage::Finals,
            ..InvitationBatch::new(
                vec![],
                vec!["family@example.test".parse().unwrap()],
                "Final photos".parse().unwrap(),
                "Enjoy!".parse().unwrap(),
            )
        };
        let announce =
            || galleries.queue_invitations(workspace, draft.id, &finals, JobOptions::default());

        sqlx::query(
            "ALTER TABLE jobs ADD CONSTRAINT reject_mail CHECK (payload->>'kind' <> 'send_email')",
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(matches!(
            announce().await,
            Err(InvitationError::Database(_))
        ));
        assert!(
            galleries
                .get(workspace, draft.id)
                .await
                .unwrap()
                .unwrap()
                .states
                .contains(&GalleryState::Shared)
        );
        sqlx::query("ALTER TABLE jobs DROP CONSTRAINT reject_mail")
            .execute(&pool)
            .await
            .unwrap();

        let (first, second) = tokio::join!(announce(), announce());
        let finals_queued = match (first, second) {
            (Ok(Some(queued)), Err(InvitationError::StageConflict))
            | (Err(InvitationError::StageConflict), Ok(Some(queued))) => queued,
            _ => panic!("exactly one delivery must succeed"),
        };
        assert_eq!(finals_queued.queued_count, 1);
        assert!(
            finals_queued
                .gallery
                .states
                .contains(&GalleryState::Delivered)
        );

        let finals_job = jobs
            .claim(&[JobKind::SendEmail], Duration::from_secs(30))
            .await
            .unwrap()
            .unwrap();
        let finals_payload: SendEmail = finals_job.payload().unwrap();
        assert_eq!(finals_payload.stage, InvitationStage::Finals);
        assert!(jobs.complete(&finals_job).await.unwrap().is_current());

        // Expired gallery refuses invitations without sharing or enqueuing.
        let expired_draft = galleries.create(workspace).await.unwrap();
        set_column(
            &pool,
            expired_draft.id,
            "expires_at",
            "(now() AT TIME ZONE 'UTC')::date - 1",
        )
        .await;
        assert!(matches!(
            galleries
                .queue_invitations(workspace, expired_draft.id, &batch, JobOptions::default())
                .await,
            Err(InvitationError::Expired)
        ));
        assert!(
            galleries
                .get(workspace, expired_draft.id)
                .await
                .unwrap()
                .unwrap()
                .share_token
                .is_none()
        );

        // Archived gallery refuses invitations.
        let archived_draft = galleries.create(workspace).await.unwrap();
        galleries
            .archive(workspace, archived_draft.id)
            .await
            .unwrap();
        assert!(matches!(
            galleries
                .queue_invitations(workspace, archived_draft.id, &batch, JobOptions::default())
                .await,
            Err(InvitationError::Archived)
        ));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn share_in_returns_none_for_missing_or_foreign_gallery_and_retries_token_collisions(
        pool: PgPool,
    ) {
        let database = Database::new(pool.clone());
        let galleries = database.repository::<GalleryRepository>();
        let (mine, other) = (workspace(&pool).await, workspace(&pool).await);
        let first = galleries.create(mine).await.unwrap();
        let second = galleries.create(mine).await.unwrap();
        let existing_token = galleries
            .share(mine, first.id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap();

        let mut tx = pool.begin().await.unwrap();

        assert_eq!(
            GalleryRepository::share_in(
                &mut tx,
                mine,
                GalleryId::generate(),
                ShareToken::generate,
            )
            .await
            .unwrap(),
            None
        );
        assert_eq!(
            GalleryRepository::share_in(&mut tx, other, second.id, ShareToken::generate)
                .await
                .unwrap(),
            None
        );

        let fresh_token = ShareToken::generate();
        let mut candidates = vec![existing_token, fresh_token.clone()].into_iter();
        let shared =
            GalleryRepository::share_in(&mut tx, mine, second.id, || candidates.next().unwrap())
                .await
                .unwrap()
                .unwrap();

        assert_eq!(shared.share_token, Some(fresh_token));
        tx.commit().await.unwrap();
    }

    #[test]
    fn invitation_batch_deserializes_deduplicates_in_place_and_rejects_unknown_fields() {
        let empty: InvitationBatch = serde_json::from_value(json!({
            "subject": "Ready",
            "message": "Hello!"
        }))
        .unwrap();

        assert!(empty.is_empty());
        assert_eq!(empty.recipient_count(), 0);
        assert!(
            serde_json::from_value::<InvitationBatch>(json!({
                "subject": "Ready",
                "message": "Hello!",
                "unexpected": true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<InvitationBatch>(json!({
                "to": [{"email": "a@example.test", "unexpected": true}],
                "subject": "Ready",
                "message": "Hello!"
            }))
            .is_err()
        );

        let batch: InvitationBatch = serde_json::from_value(json!({
            "to": [
                {"email": "Camille@Example.test", "name": "Camille"},
                {"email": "hugo@example.test"},
                {"email": "camille@example.test", "name": "Duplicate"}
            ],
            "bcc": [
                "HUGO@example.test",
                "press@example.test",
                "Press@Example.test"
            ],
            "subject": "Your gallery",
            "message": "Line 1\nLine 2"
        }))
        .unwrap();

        assert!(!batch.is_empty());
        assert_eq!(batch.recipient_count(), 3);
        assert_eq!(
            batch.to(),
            &[
                InvitationContact {
                    email: "camille@example.test".parse().unwrap(),
                    link_id: None,
                    name: Some("Camille".parse().unwrap()),
                },
                InvitationContact {
                    email: "hugo@example.test".parse().unwrap(),
                    link_id: None,
                    name: None,
                },
            ]
        );
        assert_eq!(
            batch.bcc(),
            &["press@example.test".parse::<Email>().unwrap()]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn bounded_u8_sql_decode_enforces_range_bounds(pool: PgPool) {
        let zero: FocalCoordinate = sqlx::query_scalar("SELECT 0::smallint")
            .fetch_one(&pool)
            .await
            .unwrap();
        let hundred: FocalCoordinate = sqlx::query_scalar("SELECT 100::smallint")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(zero.get(), 0);
        assert_eq!(hundred.get(), 100);

        for out_of_range in [-1i16, 101, 256] {
            let decoded = sqlx::query_scalar::<_, FocalCoordinate>("SELECT $1::smallint")
                .bind(out_of_range)
                .fetch_one(&pool)
                .await;

            assert!(decoded.is_err(), "{out_of_range} must fail SQL decode");
        }
    }
}
