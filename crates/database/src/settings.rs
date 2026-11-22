//! Workspace watermark settings and preparation of marked proof copies.
//!
//! The *watermark* is what is actually drawn on a proof: [`Watermark`]. Changing the effective
//! watermark refreshes unpublished galleries; changing proof quality refreshes all galleries.
//! A gallery switch only chooses the served pair. Clean readiness survives regeneration, while
//! marked access requires the proof's requested revision. Publication checks that revision
//! and the job claim before replacing files.

use std::{fmt, num::NonZeroU16, str::FromStr};

use pp_core::types::bounded::BoundedU8;
pub use pp_core::types::studio::{CoverSignature, Language};
use serde::{Deserialize, Serialize};
use sqlx::{
    AssertSqlSafe, FromRow, PgConnection, PgPool, Postgres, Row, Transaction, Type, types::Text,
};
use thiserror::Error;

use crate::photo::PhotoId;
use crate::{
    Repository,
    activity::ActivityOptions,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::{Gallery, GalleryChanges, GalleryError, GalleryId, GalleryRepository},
    jobs::JobOptions,
};

/// Longest studio name, in characters.
pub const STUDIO_NAME_MAX_CHARS: usize = 100;

/// Default watermark grid cell: bottom right (`8`).
pub const DEFAULT_GRID_CELL: GridCell = GridCell::constant::<8>();

/// Default watermark and cover-signature size step (`4`).
pub const DEFAULT_SIZE_STEP: SizeStep = SizeStep::constant::<4>();

/// Default watermark opacity (50%).
pub const DEFAULT_WATERMARK_OPACITY: WatermarkOpacity = WatermarkOpacity::constant::<50>();

/// Why settings were refused.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("the studio name is required when the watermark shows it")]
    NameRequired,
    #[error("the studio name is longer than {STUDIO_NAME_MAX_CHARS} characters")]
    NameTooLong,
    #[error("a logo is required when the watermark or cover signature shows it")]
    LogoRequired,
    #[error("database error")]
    Database(#[from] DatabaseError),
}

impl From<sqlx::Error> for SettingsError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error.into())
    }
}

/// What the watermark shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "watermark_mark", rename_all = "snake_case")]
pub enum WatermarkMark {
    Name,
    Logo,
    Both,
}

impl WatermarkMark {
    const fn shows_name(self) -> bool {
        matches!(self, Self::Name | Self::Both)
    }

    const fn shows_logo(self) -> bool {
        matches!(self, Self::Logo | Self::Both)
    }
}

/// Whether the watermark covers the photo or sits at one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "watermark_pattern", rename_all = "snake_case")]
pub enum WatermarkPattern {
    Repeat,
    Once,
}

/// How sharp, and how heavy, thumbnails and previews of proofs are.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "proof_quality", rename_all = "snake_case")]
pub enum ProofQuality {
    Low,
    #[default]
    Medium,
    High,
}

impl ProofQuality {
    /// Longest side of the gallery grid thumbnail, in pixels.
    pub const fn thumbnail_edge(self) -> u32 {
        match self {
            Self::Low => 640,
            Self::Medium => 800,
            Self::High => 1000,
        }
    }

    /// Longest side of the full-screen preview, in pixels: sharp on laptops and phones, too
    /// small to print.
    pub const fn preview_edge(self) -> u32 {
        match self {
            Self::Low => 1600,
            Self::Medium => 2000,
            Self::High => 2400,
        }
    }

    /// JPEG output quality.
    pub const fn jpeg_quality(self) -> i32 {
        match self {
            Self::Low => 75,
            Self::Medium => 82,
            Self::High => 88,
        }
    }
}

/// A cell of the 3 × 3 position grid, 0 (top left) to 8 (bottom right).
pub type GridCell = BoundedU8<0, 8>;

/// How large the watermark or the cover signature is: 0 (small) to 9 (large).
pub type SizeStep = BoundedU8<0, 9>;

/// Watermark opacity in whole percent, including 0 (no paint).
pub type WatermarkOpacity = BoundedU8<0, 100>;

/// A studio text color must be a `#` followed by six ASCII hex digits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("a studio text color must be in #RRGGBB format")]
pub struct InvalidWatermarkColor;

/// An RGB color that parses case-insensitively and displays as uppercase `#RRGGBB`.
///
/// ```
/// use pp_database::settings::WatermarkColor;
///
/// let color: WatermarkColor = "#a12b3c".parse().unwrap();
/// assert_eq!(color.rgb(), [161, 43, 60]);
/// assert_eq!(color.to_string(), "#A12B3C");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(try_from = "String", into = "String")]
#[sqlx(transparent, no_pg_array)]
pub struct WatermarkColor(Text<Rgb>);

impl WatermarkColor {
    pub const fn rgb(self) -> [u8; 3] {
        let Self(Text(Rgb(channels))) = self;

        channels
    }
}

impl Default for WatermarkColor {
    fn default() -> Self {
        Self(Text(Rgb([255; 3])))
    }
}

impl FromStr for WatermarkColor {
    type Err = InvalidWatermarkColor;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(|rgb| Self(Text(rgb)))
    }
}

impl fmt::Display for WatermarkColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&*self.0, formatter)
    }
}

impl TryFrom<String> for WatermarkColor {
    type Error = InvalidWatermarkColor;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<WatermarkColor> for String {
    fn from(color: WatermarkColor) -> Self {
        color.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rgb([u8; 3]);

impl FromStr for Rgb {
    type Err = InvalidWatermarkColor;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let hex = value.strip_prefix('#').ok_or(InvalidWatermarkColor)?;

        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(InvalidWatermarkColor);
        }

        let [_, r, g, b] = u32::from_str_radix(hex, 16)
            .map_err(|_| InvalidWatermarkColor)?
            .to_be_bytes();

        Ok(Self([r, g, b]))
    }
}

impl fmt::Display for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b] = self.0;

        write!(formatter, "#{r:02X}{g:02X}{b:02X}")
    }
}

/// Where a watermark goes on a photo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatermarkPlacement {
    /// Tiled over the whole photo.
    Repeat,
    /// One copy in a grid cell.
    Once(GridCell),
}

/// What is drawn on a proof. Two settings that draw the same thing are equal, so a change to a
/// setting the mark does not use (an unused logo, the position of a repeated mark) changes
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watermark {
    pub text: Option<WatermarkText>,
    /// PNG bytes.
    pub logo: Option<Vec<u8>>,
    pub placement: WatermarkPlacement,
    pub size: SizeStep,
    pub opacity: WatermarkOpacity,
}

/// The studio text and its color; neither affects a logo-only mark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatermarkText {
    pub value: String,
    pub color: WatermarkColor,
}

/// Caption placement on photographer gallery cards and photo tiles.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "studio_mode", rename_all = "snake_case")]
pub enum StudioMode {
    #[default]
    FullImage,
    Print,
}

/// Palette used by the photographer's workspace.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "studio_palette", rename_all = "snake_case")]
pub enum StudioPalette {
    #[default]
    Warm,
    Neutral,
}

/// The saved workspace settings.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct WorkspaceSettings {
    pub studio_mode: StudioMode,
    pub studio_palette: StudioPalette,
    pub studio_name: String,
    pub mark: WatermarkMark,
    pub pattern: WatermarkPattern,
    pub position: GridCell,
    pub size: SizeStep,
    pub signature: CoverSignature,
    pub signature_size: SizeStep,
    /// `None` inherits the cover's text color.
    pub signature_color: Option<WatermarkColor>,
    pub proof_quality: ProofQuality,
    pub watermark_opacity: WatermarkOpacity,
    pub watermark_text_color: WatermarkColor,
    /// Normalised PNG bytes.
    pub logo: Option<Vec<u8>>,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            studio_name: String::new(),
            studio_mode: StudioMode::default(),
            studio_palette: StudioPalette::default(),
            mark: WatermarkMark::Name,
            pattern: WatermarkPattern::Repeat,
            position: DEFAULT_GRID_CELL,
            size: DEFAULT_SIZE_STEP,
            signature: CoverSignature::Name,
            signature_size: DEFAULT_SIZE_STEP,
            signature_color: None,
            proof_quality: ProofQuality::default(),
            watermark_opacity: DEFAULT_WATERMARK_OPACITY,
            watermark_text_color: WatermarkColor::default(),
            logo: None,
        }
    }
}

impl WorkspaceSettings {
    /// What these settings draw, or `None` while there is nothing to draw yet.
    pub fn watermark(&self) -> Option<Watermark> {
        if self.watermark_opacity == WatermarkOpacity::MIN {
            return None;
        }

        let text =
            (self.mark.shows_name() && !self.studio_name.is_empty()).then(|| WatermarkText {
                value: self.studio_name.clone(),
                color: self.watermark_text_color,
            });
        let logo = self.logo.clone().filter(|_| self.mark.shows_logo());

        if text.is_none() && logo.is_none() {
            return None;
        }

        let placement = match self.pattern {
            WatermarkPattern::Repeat => WatermarkPlacement::Repeat,
            WatermarkPattern::Once => WatermarkPlacement::Once(self.position),
        };

        Some(Watermark {
            text,
            logo,
            placement,
            size: self.size,
            opacity: self.watermark_opacity,
        })
    }
}

/// What to do with the saved logo when saving settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogoChange {
    Keep,
    Remove,
    /// A normalised PNG.
    Replace(Vec<u8>),
}

/// Settings to save, all at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsUpdate {
    pub studio_mode: Option<StudioMode>,
    pub studio_palette: Option<StudioPalette>,
    pub studio_name: String,
    pub mark: WatermarkMark,
    pub pattern: WatermarkPattern,
    pub position: GridCell,
    pub size: SizeStep,
    pub signature: CoverSignature,
    pub signature_size: SizeStep,
    /// Omit to keep the current color; an explicit `None` restores inheritance.
    pub signature_color: Option<Option<WatermarkColor>>,
    pub proof_quality: ProofQuality,
    pub watermark_opacity: Option<WatermarkOpacity>,
    pub watermark_text_color: Option<WatermarkColor>,
    pub logo: LogoChange,
}

/// Identifies the work requested for a proof, independently of the gallery switch.
/// It does not identify a historical snapshot of the workspace style.
/// Finals have no watermark revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatermarkStamp(Option<i64>);

impl WatermarkStamp {
    /// SQL for the stamp of the photo `p`, which only proofs have.
    pub(crate) const SQL: &'static str = "CASE WHEN p.kind = 'proof'
         THEN p.requested_watermark_revision END";

    pub(crate) const fn new(revision: Option<i64>) -> Self {
        Self(revision)
    }

    pub const fn revision(self) -> Option<i64> {
        self.0
    }
}

/// What a photo's proofs must show right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofWatermark {
    pub stamp: WatermarkStamp,
    /// The quality to render at: the workspace's for proofs, the default for finals.
    pub quality: ProofQuality,
    /// Whether the published clean files can be reused for this render.
    pub clean_current: bool,
    /// False if a reset or delivery happened before the render could start.
    pub is_processing: bool,
    /// `None` for finals or when nothing is saved to draw.
    pub watermark: Option<Watermark>,
}

/// Data access for the workspace settings; see the module docs.
pub struct SettingsRepository<'db> {
    pool: &'db PgPool,
    jobs: JobOptions,
}

#[derive(Clone, Copy)]
enum ProofRefresh {
    WatermarkOnly,
    Quality,
}

impl<'db> Repository<'db> for SettingsRepository<'db> {
    type Options = JobOptions;

    fn open(pool: &'db PgPool, jobs: Self::Options) -> Self {
        Self { pool, jobs }
    }
}

impl SettingsRepository<'_> {
    /// The settings columns for a workspace that may have no row yet, defaulted like the table.
    const SETTINGS_OR_DEFAULTS: &'static str = "COALESCE(s.studio_name, '') AS studio_name,
        COALESCE(s.studio_mode, 'full_image') AS studio_mode,
        COALESCE(s.studio_palette, 'warm') AS studio_palette,
        COALESCE(s.mark, 'name') AS mark, COALESCE(s.pattern, 'repeat') AS pattern,
        COALESCE(s.position, 8::smallint) AS position, COALESCE(s.size, 4::smallint) AS size,
        COALESCE(s.signature, 'name') AS signature,
        COALESCE(s.signature_size, 4::smallint) AS signature_size,
        s.signature_color,
        COALESCE(s.proof_quality, 'medium') AS proof_quality, s.logo AS logo,
        COALESCE(s.watermark_opacity, 50::smallint) AS watermark_opacity,
        COALESCE(s.watermark_text_color, '#FFFFFF') AS watermark_text_color";

    /// The workspace's settings, or the defaults when none were saved.
    pub async fn settings(
        &self,
        workspace_id: WorkspaceId,
    ) -> Result<WorkspaceSettings, DatabaseError> {
        Ok(sqlx::query_as(AssertSqlSafe(format!(
            "SELECT {} FROM (SELECT $1::uuid AS id) w
             LEFT JOIN workspace_settings s ON s.workspace_id = w.id",
            Self::SETTINGS_OR_DEFAULTS
        )))
        .bind(workspace_id)
        .fetch_one(self.pool)
        .await?)
    }

    /// Saves the settings and schedules affected proofs in one transaction. Changes to the
    /// visible watermark skip delivered galleries; quality changes include them.
    ///
    /// The name is trimmed. A mark or cover signature that shows the name or logo needs one.
    pub async fn save(
        &self,
        workspace_id: WorkspaceId,
        update: SettingsUpdate,
    ) -> Result<WorkspaceSettings, SettingsError> {
        let studio_name = update.studio_name.trim().to_owned();

        if studio_name.chars().count() > STUDIO_NAME_MAX_CHARS {
            return Err(SettingsError::NameTooLong);
        }

        if update.mark.shows_name() && studio_name.is_empty() {
            return Err(SettingsError::NameRequired);
        }

        let mut transaction = self.pool.begin().await?;

        // Upload and retry admission take SHARE to stay inside the regeneration snapshot.
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
            .bind(workspace_id)
            .fetch_optional(&mut *transaction)
            .await?;

        sqlx::query(
            "INSERT INTO workspace_settings(workspace_id) VALUES ($1) ON CONFLICT DO NOTHING",
        )
        .bind(workspace_id)
        .execute(&mut *transaction)
        .await?;

        let current: WorkspaceSettings = sqlx::query_as(
            "SELECT studio_name, mark, pattern, position, size, signature, signature_size, signature_color,
                    proof_quality, logo, watermark_opacity, watermark_text_color, studio_mode, studio_palette
             FROM workspace_settings WHERE workspace_id = $1 FOR UPDATE",
        )
        .bind(workspace_id)
        .fetch_one(&mut *transaction)
        .await?;
        let logo = match update.logo {
            LogoChange::Keep => current.logo.clone(),
            LogoChange::Remove => None,
            LogoChange::Replace(png) => Some(png),
        };

        if logo.is_none() && (update.mark.shows_logo() || update.signature.shows_logo()) {
            return Err(SettingsError::LogoRequired);
        }

        let saved = WorkspaceSettings {
            studio_name,
            studio_mode: update.studio_mode.unwrap_or(current.studio_mode),
            studio_palette: update.studio_palette.unwrap_or(current.studio_palette),
            mark: update.mark,
            pattern: update.pattern,
            position: update.position,
            size: update.size,
            signature: update.signature,
            signature_size: update.signature_size,
            signature_color: update.signature_color.unwrap_or(current.signature_color),
            proof_quality: update.proof_quality,
            watermark_opacity: update
                .watermark_opacity
                .unwrap_or(current.watermark_opacity),
            watermark_text_color: update
                .watermark_text_color
                .unwrap_or(current.watermark_text_color),
            logo,
        };
        let refresh = if saved.proof_quality != current.proof_quality {
            Some(ProofRefresh::Quality)
        } else if saved.watermark() != current.watermark() {
            Some(ProofRefresh::WatermarkOnly)
        } else {
            None
        };

        let revision: i64 = sqlx::query_scalar(
            "UPDATE workspace_settings SET
                 studio_name = $2, mark = $3, pattern = $4, position = $5, size = $6,
                 signature = $7, logo = $8, proof_quality = $10, signature_size = $11,
                 watermark_revision = watermark_revision + $9, updated_at = now(),
                 watermark_opacity = $12, watermark_text_color = $13, signature_color = $14,
                 studio_mode = $15, studio_palette = $16
             WHERE workspace_id = $1 RETURNING watermark_revision",
        )
        .bind(workspace_id)
        .bind(&saved.studio_name)
        .bind(saved.mark)
        .bind(saved.pattern)
        .bind(saved.position)
        .bind(saved.size)
        .bind(saved.signature)
        .bind(&saved.logo)
        .bind(i64::from(refresh.is_some()))
        .bind(saved.proof_quality)
        .bind(saved.signature_size)
        .bind(saved.watermark_opacity)
        .bind(saved.watermark_text_color)
        .bind(saved.signature_color)
        .bind(saved.studio_mode)
        .bind(saved.studio_palette)
        .fetch_one(&mut *transaction)
        .await?;

        if let Some(refresh) = refresh {
            Self::regenerate_in(
                &mut transaction,
                workspace_id,
                self.jobs.max_attempts,
                refresh,
                revision,
            )
            .await?;
        }

        transaction.commit().await?;

        Ok(saved)
    }

    /// Saves studio presentation independently of branding, without scheduling proof work.
    pub async fn save_display(
        &self,
        workspace_id: WorkspaceId,
        mode: StudioMode,
        palette: StudioPalette,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "INSERT INTO workspace_settings(workspace_id, studio_mode, studio_palette) VALUES ($1, $2, $3)
             ON CONFLICT (workspace_id) DO UPDATE SET studio_mode = EXCLUDED.studio_mode,
                 studio_palette = EXCLUDED.studio_palette, updated_at = now()",
        )
        .bind(workspace_id)
        .bind(mode)
        .bind(palette)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// The workspace's language, English when none was saved.
    pub async fn language(&self, workspace_id: WorkspaceId) -> Result<Language, DatabaseError> {
        Ok(sqlx::query_scalar(
            "SELECT COALESCE(s.language, 'en') FROM (SELECT $1::uuid AS id) w
             LEFT JOIN workspace_settings s ON s.workspace_id = w.id",
        )
        .bind(workspace_id)
        .fetch_one(self.pool)
        .await?)
    }

    /// Saves the language alone: the watermark and its revision stay as they are.
    pub async fn save_language(
        &self,
        workspace_id: WorkspaceId,
        language: Language,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            "INSERT INTO workspace_settings(workspace_id, language) VALUES ($1, $2)
             ON CONFLICT (workspace_id)
             DO UPDATE SET language = EXCLUDED.language, updated_at = now()",
        )
        .bind(workspace_id)
        .bind(language)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// The language of the installation's only workspace, or `None` before setup created it.
    pub async fn installation_language(&self) -> Result<Option<Language>, DatabaseError> {
        Ok(sqlx::query_scalar(
            "SELECT COALESCE(s.language, 'en') FROM workspaces w
             LEFT JOIN workspace_settings s ON s.workspace_id = w.id
             ORDER BY w.id LIMIT 1",
        )
        .fetch_optional(self.pool)
        .await?)
    }

    /// Updates the gallery without changing prepared images or enqueueing processing.
    pub async fn update_gallery(
        &self,
        workspace_id: WorkspaceId,
        id: GalleryId,
        changes: &GalleryChanges,
    ) -> Result<Option<Gallery>, GalleryError> {
        GalleryRepository::open(self.pool, ActivityOptions::default())
            .update(workspace_id, id, changes)
            .await
    }

    /// Fetches the render inputs and the photo's current target in one read, or `None` if
    /// it no longer exists.
    pub async fn proof_watermark(
        &self,
        photo: PhotoId,
    ) -> Result<Option<ProofWatermark>, DatabaseError> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT p.kind = 'proof' AS enabled,
                    p.status = 'processing' AS is_processing,
                    p.clean_ready AND (p.watermark_only_pending OR
                        p.clean_quality = COALESCE(s.proof_quality, 'medium')) AS clean_current,
                    {stamp} AS stamp, {settings}
             FROM photos p
             LEFT JOIN workspace_settings s ON s.workspace_id = p.workspace_id
             WHERE p.id = $1",
            stamp = WatermarkStamp::SQL,
            settings = Self::SETTINGS_OR_DEFAULTS,
        )))
        .bind(photo)
        .fetch_optional(self.pool)
        .await?;

        row.map(|row| {
            let enabled: bool = row.try_get("enabled")?;
            let stamp = WatermarkStamp::new(row.try_get("stamp")?);
            let settings = WorkspaceSettings::from_row(&row)?;
            let watermark = settings.watermark().filter(|_| enabled);

            Ok(ProofWatermark {
                stamp,
                quality: if enabled {
                    settings.proof_quality
                } else {
                    ProofQuality::default()
                },
                clean_current: row
                    .try_get::<Option<bool>, _>("clean_current")?
                    .unwrap_or(false),
                is_processing: row.try_get("is_processing")?,
                watermark,
            })
        })
        .transpose()
        .map_err(|error: sqlx::Error| error.into())
    }

    /// Targets all affected proofs, but creates jobs only for `ready` and `processing`
    /// proofs. Photo locks are ordered and existing jobs are not locked here, so a worker
    /// holding its job can safely race the reset.
    async fn regenerate_in(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: WorkspaceId,
        max_attempts: NonZeroU16,
        refresh: ProofRefresh,
        revision: i64,
    ) -> Result<(), sqlx::Error> {
        let connection: &mut PgConnection = transaction;
        sqlx::query(
            "WITH locked AS (
                 SELECT p.id, p.status FROM photos p JOIN galleries g ON g.id = p.gallery_id
                 WHERE p.workspace_id = $1 AND p.kind = 'proof'
                   AND ($3 OR g.delivered_at IS NULL)
                 ORDER BY p.id
                 FOR UPDATE OF p
             ), reset AS (
                 UPDATE photos p SET requested_watermark_revision = $4,
                     watermark_only_pending = NOT $3 AND (p.watermark_only_pending OR (
                         locked.status = 'ready' AND p.clean_ready AND p.watermark_revision IS NOT NULL)),
                     status = CASE WHEN locked.status IN ('ready', 'processing')
                         THEN 'waiting' ELSE locked.status END,
                     failure_reason = CASE WHEN locked.status = 'failed' THEN p.failure_reason END
                 FROM locked WHERE p.id = locked.id
                 RETURNING p.id, locked.status AS previous_status
             )
             INSERT INTO jobs(id, payload, max_attempts)
             SELECT gen_random_uuid(),
                    jsonb_build_object('kind', 'process_photo',
                                       'payload', jsonb_build_object('photo_id', id)),
                    $2
             FROM reset WHERE previous_status IN ('ready', 'processing')",
        )
        .bind(workspace_id)
        .bind(i32::from(max_attempts.get()))
        .bind(matches!(refresh, ProofRefresh::Quality))
        .bind(revision)
        .execute(connection)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use pp_core::types::generate::Generate;
    use sqlx::PgPool;
    use tokio::{task, time};

    use super::*;
    use crate::{
        Database,
        gallery::Delivery,
        jobs::{Job, JobKind, JobRepository, PhotoResult, PhotoUpdate, ProcessPhoto, Publication},
        photo::{
            FavoriteChange, NewPhoto, PhotoDimensions, PhotoKind, PhotoRepository, PhotoStatus,
            Watermarked,
        },
        photo_retry::PhotoRetry,
        tests::TestRepository,
    };

    const LEASE: Duration = Duration::from_secs(30);
    const KINDS: &[JobKind] = &[JobKind::ProcessPhoto];
    const PNG: &[u8] = b"\x89PNG logo";

    struct Fixture {
        pool: PgPool,
        database: Database,
        workspace_id: WorkspaceId,
    }

    impl Fixture {
        async fn new(pool: PgPool) -> Self {
            let database = Database::new(pool.clone());
            let workspace_id = database
                .repository::<TestRepository>()
                .create_workspace()
                .await
                .unwrap();

            Self {
                pool,
                database,
                workspace_id,
            }
        }

        fn watermarks(&self) -> SettingsRepository<'_> {
            self.database.repository::<SettingsRepository>()
        }

        async fn gallery(&self, watermarked: bool) -> GalleryId {
            let galleries = self.database.repository::<GalleryRepository>();
            let id = galleries.create(self.workspace_id).await.unwrap().id;

            if watermarked {
                self.watermarks()
                    .update_gallery(
                        self.workspace_id,
                        id,
                        &GalleryChanges {
                            watermark_enabled: Some(true),
                            ..GalleryChanges::default()
                        },
                    )
                    .await
                    .unwrap();
            }

            id
        }

        /// A proof in `status`, with no job.
        async fn photo(&self, gallery_id: GalleryId, status: PhotoStatus) -> PhotoId {
            self.photo_of_kind(gallery_id, PhotoKind::Proof, status)
                .await
        }

        async fn photo_of_kind(
            &self,
            gallery_id: GalleryId,
            kind: PhotoKind,
            status: PhotoStatus,
        ) -> PhotoId {
            let id = PhotoId::generate();
            let photo = NewPhoto {
                id,
                workspace_id: self.workspace_id,
                gallery_id,
                original_name: "proof.jpg".to_owned(),
                size: 1024.try_into().unwrap(),
                dimensions: PhotoDimensions {
                    width: 20.try_into().unwrap(),
                    height: 10.try_into().unwrap(),
                },
                kind,
            };
            let mut transaction = self.pool.begin().await.unwrap();

            self.database
                .repository::<PhotoRepository>()
                .insert_in(&mut transaction, &photo)
                .await
                .unwrap();
            transaction.commit().await.unwrap();
            self.database
                .repository::<TestRepository>()
                .force_photo_status(id, &status)
                .await
                .unwrap();

            id
        }

        async fn proof(&self, id: PhotoId) -> Option<ProofWatermark> {
            self.watermarks().proof_watermark(id).await.unwrap()
        }

        async fn share(&self, gallery: GalleryId) {
            self.database
                .repository::<GalleryRepository>()
                .share(self.workspace_id, gallery)
                .await
                .unwrap()
                .unwrap();
        }

        async fn deliver(&self, gallery: GalleryId) -> Delivery {
            self.database
                .repository::<GalleryRepository>()
                .deliver(self.workspace_id, gallery)
                .await
                .unwrap()
        }

        async fn status(&self, id: PhotoId) -> PhotoStatus {
            self.status_of_kind(id, PhotoKind::Proof).await
        }

        async fn status_of_kind(&self, id: PhotoId, kind: PhotoKind) -> PhotoStatus {
            let photos = self.database.repository::<PhotoRepository>();
            let location = photos.locate(id).await.unwrap().unwrap();

            photos
                .get(location.workspace_id, location.gallery_id, id, kind)
                .await
                .unwrap()
                .unwrap()
                .status
        }

        async fn jobs(&self, id: PhotoId) -> usize {
            self.database
                .repository::<TestRepository>()
                .photo_jobs(id)
                .await
                .unwrap()
                .len()
        }

        async fn save(&self, update: SettingsUpdate) -> Result<WorkspaceSettings, SettingsError> {
            self.watermarks().save(self.workspace_id, update).await
        }
    }

    fn update(name: &str) -> SettingsUpdate {
        SettingsUpdate {
            studio_name: name.to_owned(),
            studio_mode: None,
            studio_palette: None,
            mark: WatermarkMark::Name,
            pattern: WatermarkPattern::Repeat,
            position: DEFAULT_GRID_CELL,
            size: SizeStep::constant::<2>(),
            signature: CoverSignature::Name,
            signature_size: DEFAULT_SIZE_STEP,
            signature_color: None,
            proof_quality: ProofQuality::Medium,
            watermark_opacity: None,
            watermark_text_color: None,
            logo: LogoChange::Keep,
        }
    }

    #[test]
    fn watermark_appearance_only_compares_visible_inputs() {
        let logo_only = WorkspaceSettings {
            studio_name: "Studio".into(),
            mark: WatermarkMark::Logo,
            logo: Some(PNG.to_vec()),
            ..WorkspaceSettings::default()
        };
        let same_logo_new_text_color = WorkspaceSettings {
            watermark_text_color: "#C04020".parse().unwrap(),
            ..logo_only.clone()
        };
        assert_eq!(logo_only.watermark(), same_logo_new_text_color.watermark());

        let text_only = WorkspaceSettings {
            mark: WatermarkMark::Name,
            ..logo_only.clone()
        };
        let recolored = WorkspaceSettings {
            watermark_text_color: "#C04020".parse().unwrap(),
            ..text_only.clone()
        };
        assert_ne!(text_only.watermark(), recolored.watermark());

        for mark in [
            WatermarkMark::Name,
            WatermarkMark::Logo,
            WatermarkMark::Both,
        ] {
            for pattern in [WatermarkPattern::Repeat, WatermarkPattern::Once] {
                let zero = WorkspaceSettings {
                    watermark_opacity: WatermarkOpacity::MIN,
                    mark,
                    pattern,
                    ..logo_only.clone()
                };
                assert!(zero.watermark().is_none());
            }
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn settings_default_until_saved(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        assert_eq!(
            fixture
                .watermarks()
                .settings(fixture.workspace_id)
                .await
                .unwrap(),
            WorkspaceSettings::default()
        );
        assert_eq!(WorkspaceSettings::default().watermark(), None);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn language_defaults_to_english_without_a_settings_row(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let language = fixture
            .watermarks()
            .language(fixture.workspace_id)
            .await
            .unwrap();

        assert_eq!(language, Language::En);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn saving_the_language_keeps_the_watermark_revision(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.save(update("Studio")).await.unwrap();

        let revision = || {
            sqlx::query_scalar::<_, i64>(
                "SELECT watermark_revision FROM workspace_settings WHERE workspace_id = $1",
            )
            .bind(fixture.workspace_id)
            .fetch_one(&fixture.pool)
        };
        let before = revision().await.unwrap();

        fixture
            .watermarks()
            .save_language(fixture.workspace_id, Language::Fr)
            .await
            .unwrap();

        let language = fixture
            .watermarks()
            .language(fixture.workspace_id)
            .await
            .unwrap();

        assert_eq!(language, Language::Fr);
        assert_eq!(revision().await.unwrap(), before);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn installation_language_is_none_before_setup_then_the_workspace_language(pool: PgPool) {
        let database = Database::new(pool.clone());
        let watermarks = database.repository::<SettingsRepository>();

        assert_eq!(watermarks.installation_language().await.unwrap(), None);

        let fixture = Fixture::new(pool).await;

        assert_eq!(
            watermarks.installation_language().await.unwrap(),
            Some(Language::En)
        );

        fixture
            .watermarks()
            .save_language(fixture.workspace_id, Language::Fr)
            .await
            .unwrap();

        assert_eq!(
            watermarks.installation_language().await.unwrap(),
            Some(Language::Fr)
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn saved_settings_round_trip_with_a_trimmed_name(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let saved = fixture
            .save(SettingsUpdate {
                mark: WatermarkMark::Both,
                pattern: WatermarkPattern::Once,
                position: GridCell::try_from(4).unwrap(),
                size: SizeStep::constant::<4>(),
                signature: CoverSignature::None,
                signature_size: SizeStep::constant::<7>(),
                logo: LogoChange::Replace(PNG.to_vec()),
                ..update("  Studio Lumière ")
            })
            .await
            .unwrap();
        let read = fixture
            .watermarks()
            .settings(fixture.workspace_id)
            .await
            .unwrap();

        assert_eq!(saved.studio_name, "Studio Lumière");
        assert_eq!(read, saved);
        assert_eq!(
            read.watermark(),
            Some(Watermark {
                text: Some(WatermarkText {
                    value: "Studio Lumière".to_owned(),
                    color: WatermarkColor::default(),
                }),
                logo: Some(PNG.to_vec()),
                placement: WatermarkPlacement::Once(GridCell::try_from(4).unwrap()),
                size: SizeStep::constant::<4>(),
                opacity: DEFAULT_WATERMARK_OPACITY,
            })
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn settings_that_cannot_draw_what_they_promise_are_refused(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        assert!(matches!(
            fixture.save(update("   ")).await,
            Err(SettingsError::NameRequired)
        ));
        assert!(matches!(
            fixture.save(update(&"x".repeat(101))).await,
            Err(SettingsError::NameTooLong)
        ));
        assert!(matches!(
            fixture
                .save(SettingsUpdate {
                    mark: WatermarkMark::Logo,
                    ..update("Studio")
                })
                .await,
            Err(SettingsError::LogoRequired)
        ));
        assert!(matches!(
            fixture
                .save(SettingsUpdate {
                    signature: CoverSignature::Logo,
                    ..update("Studio")
                })
                .await,
            Err(SettingsError::LogoRequired)
        ));
        assert_eq!(
            fixture
                .watermarks()
                .settings(fixture.workspace_id)
                .await
                .unwrap(),
            WorkspaceSettings::default()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn removing_the_logo_is_refused_while_the_mark_uses_it(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture
            .save(SettingsUpdate {
                mark: WatermarkMark::Logo,
                logo: LogoChange::Replace(PNG.to_vec()),
                ..update("")
            })
            .await
            .unwrap();

        assert!(matches!(
            fixture
                .save(SettingsUpdate {
                    mark: WatermarkMark::Logo,
                    logo: LogoChange::Remove,
                    ..update("")
                })
                .await,
            Err(SettingsError::LogoRequired)
        ));
        assert!(
            fixture
                .save(SettingsUpdate {
                    logo: LogoChange::Remove,
                    ..update("Studio")
                })
                .await
                .unwrap()
                .logo
                .is_none()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn watermark_changes_regenerate_only_draft_and_prepublish(pool: PgPool) {
        let fixture = Fixture::new(pool.clone()).await;
        let other = Fixture::new(pool.clone()).await;
        fixture.save(update("Before")).await.unwrap();
        let foreign = other
            .photo(other.gallery(true).await, PhotoStatus::Ready)
            .await;
        let mut proofs = Vec::new();

        for (stage, flagged) in [
            (0, false),
            (1, false),
            (0, true),
            (1, true),
            (2, false),
            (2, true),
        ] {
            let gallery = fixture.gallery(stage != 0).await;
            let photos = fixture.database.repository::<PhotoRepository>();
            let photo = fixture.photo(gallery, PhotoStatus::Waiting).await;
            photos.mark_ready(photo).await.unwrap();
            let location = photos.locate(photo).await.unwrap().unwrap();
            assert_eq!(
                photos
                    .set_favorite(location, photo, true, PhotoKind::Proof, Watermarked(true))
                    .await
                    .unwrap(),
                FavoriteChange::Applied
            );

            if stage > 0 {
                fixture.share(gallery).await;
            }

            if stage == 2 {
                assert!(matches!(
                    fixture.deliver(gallery).await,
                    Delivery::Delivered(_)
                ));
            }

            if flagged {
                sqlx::query("UPDATE galleries SET archived_at = now(), expires_at = current_date - 1 WHERE id = $1")
                    .bind(gallery).execute(&pool).await.unwrap();
            }

            proofs.push((
                stage,
                location,
                photo,
                photos
                    .get(fixture.workspace_id, gallery, photo, PhotoKind::Proof)
                    .await
                    .unwrap()
                    .unwrap(),
                fixture.proof(photo).await.unwrap().stamp,
            ));
        }

        fixture.save(update("After")).await.unwrap();

        let photos = fixture.database.repository::<PhotoRepository>();
        for (stage, location, photo, before, stamp) in proofs {
            assert_eq!(
                fixture.jobs(photo).await,
                usize::from(stage != 2),
                "stage {stage}"
            );
            assert!(
                photos
                    .is_ready(location, photo, PhotoKind::Proof, Watermarked(false))
                    .await
                    .unwrap()
            );
            assert_eq!(
                photos
                    .is_ready(location, photo, PhotoKind::Proof, Watermarked(true))
                    .await
                    .unwrap(),
                stage == 2
            );
            if stage == 2 {
                assert_eq!(fixture.proof(photo).await.unwrap().stamp, stamp);
                assert_eq!(
                    photos
                        .get(
                            fixture.workspace_id,
                            location.gallery_id,
                            photo,
                            PhotoKind::Proof
                        )
                        .await
                        .unwrap()
                        .unwrap(),
                    before
                );
            } else {
                assert_eq!(fixture.status(photo).await, PhotoStatus::Waiting);
                assert!(photos.mark_ready(photo).await.unwrap());
                assert!(
                    photos
                        .is_ready(location, photo, PhotoKind::Proof, Watermarked(true))
                        .await
                        .unwrap()
                );
            }
        }
        assert_eq!(other.jobs(foreign).await, 0);
        assert_eq!(other.status(foreign).await, PhotoStatus::Ready);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn delivery_restores_only_failed_marked_refreshes(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        fixture.share(gallery).await;
        let ready = fixture.photo(gallery, PhotoStatus::Ready).await;
        let failed = fixture
            .photo(gallery, PhotoStatus::Failed("Initial upload".into()))
            .await;
        let stamp = fixture.proof(ready).await.unwrap().stamp;
        fixture.save(update("Studio")).await.unwrap();
        let jobs = fixture.database.repository::<JobRepository>();
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        let (completion, update) = jobs
            .complete_photo(&job, ready, PhotoResult::Failed("Refresh failed"))
            .await
            .unwrap();
        assert!(completion.is_current());
        assert_eq!(update, PhotoUpdate::Saved);

        assert!(matches!(
            fixture.deliver(gallery).await,
            Delivery::Delivered(_)
        ));
        assert_eq!(fixture.status(ready).await, PhotoStatus::Ready);
        assert_eq!(
            fixture.status(failed).await,
            PhotoStatus::Failed("Initial upload".into())
        );
        assert_eq!(fixture.proof(ready).await.unwrap().stamp, stamp);
        assert_eq!(fixture.jobs(ready).await, 0);
        assert_eq!(fixture.jobs(failed).await, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn publication_that_wins_the_lock_finishes_before_delivery(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        fixture.share(gallery).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;
        fixture.save(update("Studio")).await.unwrap();
        let proof = fixture.proof(photo).await.unwrap();
        assert!(
            proof.clean_current,
            "a scoped refresh reuses even a legacy clean pair"
        );
        let stamp = proof.stamp;
        let jobs = fixture.database.repository::<JobRepository>();
        let job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        let Publication::Open(publishing) =
            jobs.begin_publication(&job, photo, stamp).await.unwrap()
        else {
            panic!("current render must own publication");
        };
        let delivery = fixture.deliver(gallery);
        tokio::pin!(delivery);
        let delivered_early = tokio::select! {
            delivered = &mut delivery => Some(delivered),
            () = wait_for_lock_waiters(&fixture.pool, 1) => None,
        };
        let (completion, update) = publishing.finish().await.unwrap();
        assert!(completion.is_current());
        assert_eq!(update, PhotoUpdate::Saved);
        assert!(
            delivered_early.is_none(),
            "delivery did not wait for the publication owner"
        );
        assert!(matches!(delivery.await, Delivery::Delivered(_)));
        assert_eq!(fixture.status(photo).await, PhotoStatus::Ready);
        assert_eq!(fixture.proof(photo).await.unwrap().stamp, stamp);
        assert_eq!(fixture.jobs(photo).await, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn first_favorite_can_record_activity_while_delivery_waits_for_the_gallery(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        fixture.share(gallery).await;
        let mut favorite = fixture.pool.begin().await.unwrap();

        // A first favorite holds this gallery lock before inserting its activity row.
        sqlx::query("SELECT id FROM galleries WHERE id = $1 FOR SHARE")
            .bind(gallery)
            .fetch_one(&mut *favorite)
            .await
            .unwrap();
        let delivery = fixture.deliver(gallery);
        tokio::pin!(delivery);
        tokio::select! {
            _ = &mut delivery => panic!("delivery did not wait for the favorite's gallery lock"),
            () = wait_for_lock_waiters(&fixture.pool, 1) => {}
        }

        sqlx::query("SET LOCAL lock_timeout = '500ms'")
            .execute(&mut *favorite)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO activity_events (workspace_id, gallery_id, kind, count)
             VALUES ($1, $2, 'favorite', 1)",
        )
        .bind(fixture.workspace_id)
        .bind(gallery)
        .execute(&mut *favorite)
        .await
        .expect("delivery must not prevent the favorite's workspace FK check");
        favorite.commit().await.unwrap();

        assert!(matches!(delivery.await, Delivery::Delivered(_)));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_pending_hero_waits_for_workspace_admission(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        fixture.share(gallery).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;
        sqlx::query("UPDATE galleries SET hero_id = $1 WHERE id = $2")
            .bind(photo)
            .bind(gallery)
            .execute(&fixture.pool)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE photos SET status = 'failed', watermark_only_pending = true,
                failure_reason = 'refresh failed' WHERE id = $1",
        )
        .bind(photo)
        .execute(&fixture.pool)
        .await
        .unwrap();
        assert_eq!(fixture.jobs(photo).await, 0);

        let mut delivery = fixture.pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR NO KEY UPDATE")
            .bind(fixture.workspace_id)
            .fetch_one(&mut *delivery)
            .await
            .unwrap();
        let deletions = fixture
            .database
            .repository::<crate::workspace_deletion::WorkspaceDeletion>();
        let ids = [photo];
        let deleting =
            deletions.delete_photos(fixture.workspace_id, gallery, PhotoKind::Proof, &ids);
        tokio::pin!(deleting);
        tokio::select! {
            _ = &mut deleting => panic!("hero deletion must wait before taking photo locks"),
            () = wait_for_lock_waiters(&fixture.pool, 1) => {}
        }
        sqlx::query("SELECT id FROM photos WHERE id = $1 FOR UPDATE NOWAIT")
            .bind(photo)
            .fetch_one(&mut *delivery)
            .await
            .unwrap();
        delivery.commit().await.unwrap();
        assert_eq!(deleting.await.unwrap(), ids);
        assert!(matches!(
            fixture.deliver(gallery).await,
            Delivery::Delivered(_)
        ));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn deleting_a_proof_hero_waits_for_the_gallery_before_locking_the_photo(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;
        let galleries = fixture.database.repository::<GalleryRepository>();
        galleries
            .update(
                fixture.workspace_id,
                gallery,
                &GalleryChanges {
                    hero_id: Some(Some(photo)),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap()
            .unwrap();
        fixture.share(gallery).await;

        let mut favorite = fixture.pool.begin().await.unwrap();

        // A favorite takes the gallery lock before reaching the proof row.
        sqlx::query("SELECT id FROM galleries WHERE id = $1 FOR SHARE")
            .bind(gallery)
            .fetch_one(&mut *favorite)
            .await
            .unwrap();

        let deletions = fixture
            .database
            .repository::<crate::workspace_deletion::WorkspaceDeletion>();
        let ids = [photo];
        let deleting =
            deletions.delete_photos(fixture.workspace_id, gallery, PhotoKind::Proof, &ids);

        tokio::pin!(deleting);
        tokio::select! {
            _ = &mut deleting => panic!("hero deletion must wait for the gallery lock"),
            () = wait_for_lock_waiters(&fixture.pool, 1) => {}
        }

        // Do not wait on the photo; the invariant is that its row is still lockable.
        let lockable: Option<PhotoId> =
            sqlx::query_scalar("SELECT id FROM photos WHERE id = $1 FOR UPDATE SKIP LOCKED")
                .bind(photo)
                .fetch_optional(&mut *favorite)
                .await
                .unwrap();
        favorite.commit().await.unwrap();

        assert_eq!(deleting.await.unwrap(), ids);
        assert_eq!(
            lockable,
            Some(photo),
            "deletion must not hold the proof row while waiting for the gallery"
        );
        assert_eq!(
            galleries
                .get(fixture.workspace_id, gallery)
                .await
                .unwrap()
                .unwrap()
                .hero_id,
            None
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn assigning_a_proof_hero_waits_for_the_gallery_before_locking_the_photo(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;
        let galleries = fixture.database.repository::<GalleryRepository>();
        let mut gallery_holder = fixture.pool.begin().await.unwrap();

        sqlx::query("SELECT id FROM galleries WHERE id = $1 FOR NO KEY UPDATE")
            .bind(gallery)
            .fetch_one(&mut *gallery_holder)
            .await
            .unwrap();

        let changes = GalleryChanges {
            hero_id: Some(Some(photo)),
            ..GalleryChanges::default()
        };
        let assigning = galleries.update(fixture.workspace_id, gallery, &changes);

        tokio::pin!(assigning);
        tokio::select! {
            _ = &mut assigning => panic!("hero assignment must wait for the gallery lock"),
            () = wait_for_lock_waiters(&fixture.pool, 1) => {}
        }

        let lockable: Option<PhotoId> =
            sqlx::query_scalar("SELECT id FROM photos WHERE id = $1 FOR UPDATE SKIP LOCKED")
                .bind(photo)
                .fetch_optional(&mut *gallery_holder)
                .await
                .unwrap();
        gallery_holder.commit().await.unwrap();

        assert_eq!(assigning.await.unwrap().unwrap().hero_id, Some(photo));
        assert_eq!(
            lockable,
            Some(photo),
            "hero assignment must not hold the proof row while waiting for the gallery"
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_new_proof_quality_regenerates_every_gallery_and_finals_keep_the_default(
        pool: PgPool,
    ) {
        let fixture = Fixture::new(pool).await;
        let on = fixture.gallery(true).await;
        let off = fixture.gallery(false).await;
        let marked = fixture.photo(on, PhotoStatus::Ready).await;
        let unmarked = fixture.photo(off, PhotoStatus::Ready).await;
        let delivered = fixture.gallery(true).await;
        fixture.share(delivered).await;
        fixture.deliver(delivered).await;
        let published = fixture.photo(delivered, PhotoStatus::Ready).await;
        let finished = fixture
            .photo_of_kind(on, PhotoKind::Final, PhotoStatus::Ready)
            .await;

        fixture
            .save(SettingsUpdate {
                proof_quality: ProofQuality::High,
                ..update("Studio")
            })
            .await
            .unwrap();

        for id in [marked, unmarked, published] {
            assert_eq!(fixture.status(id).await, PhotoStatus::Waiting);
            assert_eq!(fixture.jobs(id).await, 1);
        }

        assert_eq!(fixture.jobs(finished).await, 0);
        assert_eq!(
            fixture.proof(marked).await.unwrap().quality,
            ProofQuality::High
        );
        assert_eq!(
            fixture.proof(finished).await.unwrap().quality,
            ProofQuality::Medium
        );
        let photos = fixture.database.repository::<PhotoRepository>();
        assert!(photos.mark_ready(published).await.unwrap());
        assert!(fixture.proof(published).await.unwrap().clean_current);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn finals_are_never_watermarked_or_regenerated(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let finished = fixture
            .photo_of_kind(gallery, PhotoKind::Final, PhotoStatus::Ready)
            .await;

        fixture.save(update("Studio")).await.unwrap();

        assert_eq!(
            fixture.status_of_kind(finished, PhotoKind::Final).await,
            PhotoStatus::Ready
        );
        assert_eq!(fixture.jobs(finished).await, 0);

        let proof = fixture.proof(finished).await.unwrap();

        assert_eq!(proof.watermark, None);
        assert_eq!(proof.stamp, WatermarkStamp::new(None));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn settings_that_change_nothing_drawn_leave_proofs_alone(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;

        fixture.save(update("Studio")).await.unwrap();

        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;

        let saved = fixture
            .save(SettingsUpdate {
                signature: CoverSignature::None,
                signature_size: SizeStep::constant::<9>(),
                signature_color: Some(Some("#C04020".parse().unwrap())),
                position: GridCell::try_from(0).unwrap(),
                logo: LogoChange::Replace(PNG.to_vec()),
                ..update("Studio")
            })
            .await
            .unwrap();

        assert_eq!(saved.signature_color, Some("#C04020".parse().unwrap()));
        assert_eq!(fixture.status(photo).await, PhotoStatus::Ready);
        assert_eq!(fixture.jobs(photo).await, 0);

        fixture
            .save(SettingsUpdate {
                size: SizeStep::constant::<3>(),
                ..update("Studio")
            })
            .await
            .unwrap();

        assert_eq!(fixture.status(photo).await, PhotoStatus::Waiting);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn inactive_color_and_zero_opacity_edits_do_not_queue_work(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let logo = SettingsUpdate {
            mark: WatermarkMark::Logo,
            logo: LogoChange::Replace(PNG.to_vec()),
            ..update("Studio")
        };
        fixture.save(logo.clone()).await.unwrap();
        let first = fixture.photo(gallery, PhotoStatus::Ready).await;
        let color = "#C04020".parse().unwrap();
        let saved = fixture
            .save(SettingsUpdate {
                watermark_text_color: Some(color),
                ..logo.clone()
            })
            .await
            .unwrap();
        assert_eq!(saved.watermark_text_color, color);
        assert_eq!(fixture.jobs(first).await, 0);

        fixture
            .save(SettingsUpdate {
                watermark_opacity: Some(WatermarkOpacity::MIN),
                ..logo
            })
            .await
            .unwrap();
        let second = fixture.photo(gallery, PhotoStatus::Ready).await;
        let saved = fixture
            .save(SettingsUpdate {
                watermark_text_color: Some(WatermarkColor::default()),
                pattern: WatermarkPattern::Once,
                ..update("Another Studio")
            })
            .await
            .unwrap();
        assert_eq!(saved.watermark_opacity, WatermarkOpacity::MIN);
        assert_eq!(saved.pattern, WatermarkPattern::Once);
        assert_eq!(fixture.jobs(first).await, 1);
        assert_eq!(fixture.jobs(second).await, 0);
    }

    #[sqlx::test(migrations = false)]
    async fn watermark_scope_migration_preserves_existing_assets_and_jobs(pool: PgPool) {
        for migration in crate::MIGRATOR
            .iter()
            .filter(|migration| migration.version <= 38)
        {
            sqlx::raw_sql(migration.sql.clone())
                .execute(&pool)
                .await
                .unwrap();
        }

        let fixture = Fixture::new(pool.clone()).await;
        let gallery = fixture.gallery(true).await;
        sqlx::query(
            "INSERT INTO workspace_settings (workspace_id, studio_name, watermark_revision)
                     VALUES ($1, 'Studio', 7)",
        )
        .bind(fixture.workspace_id)
        .execute(&pool)
        .await
        .unwrap();

        let ready = PhotoId::generate();
        let waiting = PhotoId::generate();
        let final_id = PhotoId::generate();
        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height,
                kind, status, clean_ready, watermark_revision, revision)
             VALUES ($1, $4, $5, 'proof.jpg', 10, 10, 10, 'proof', 'ready', true, 7, 11),
                    ($2, $4, $5, 'new.jpg', 10, 10, 10, 'proof', 'waiting', false, NULL, 0),
                    ($3, $4, $5, 'final.jpg', 10, 10, 10, 'final', 'ready', true, NULL, 2)",
        )
        .bind(ready)
        .bind(waiting)
        .bind(final_id)
        .bind(fixture.workspace_id)
        .bind(gallery)
        .execute(&pool)
        .await
        .unwrap();
        let payload =
            serde_json::json!({"kind": "process_photo", "payload": {"photo_id": waiting}});
        sqlx::query(
            "INSERT INTO jobs (id, payload, max_attempts) VALUES (gen_random_uuid(), $1, 5)",
        )
        .bind(&payload)
        .execute(&pool)
        .await
        .unwrap();

        for migration in crate::MIGRATOR
            .iter()
            .filter(|migration| migration.version > 38)
        {
            sqlx::raw_sql(migration.sql.clone())
                .execute(&pool)
                .await
                .unwrap();
        }

        let appearance: (WatermarkOpacity, WatermarkColor) = sqlx::query_as(
            "SELECT watermark_opacity, watermark_text_color FROM workspace_settings
             WHERE workspace_id = $1",
        )
        .bind(fixture.workspace_id)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(
            appearance,
            (DEFAULT_WATERMARK_OPACITY, WatermarkColor::default())
        );
        for (photo, expected) in [
            (ready, (Some(7), 7, "ready", 11)),
            (waiting, (None, 7, "waiting", 0)),
            (final_id, (None, 0, "ready", 2)),
        ] {
            let (marked, requested, status, assets, pending): (
                Option<i64>,
                i64,
                String,
                i64,
                bool,
            ) = sqlx::query_as(
                "SELECT watermark_revision, requested_watermark_revision,
                    status::text, revision, watermark_only_pending FROM photos WHERE id = $1",
            )
            .bind(photo)
            .fetch_one(&pool)
            .await
            .unwrap();
            assert_eq!((marked, requested, status.as_str(), assets), expected);
            assert!(!pending);
        }
        let photos = fixture.database.repository::<PhotoRepository>();
        let location = photos.locate(ready).await.unwrap().unwrap();
        assert_eq!(
            photos
                .ready_derivative(location, ready, PhotoKind::Proof, Watermarked(true))
                .await
                .unwrap()
                .expect("ready legacy marked files must remain readable")
                .marked_file_generation,
            None
        );
        assert_eq!(
            fixture.proof(final_id).await.unwrap().stamp.revision(),
            None
        );
        assert_eq!(
            sqlx::query_scalar::<_, serde_json::Value>("SELECT payload FROM jobs")
                .fetch_all(&pool)
                .await
                .unwrap(),
            vec![payload]
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn studio_display_migration_preserves_branding_and_display_saves_leave_proofs_alone(
        pool: PgPool,
    ) {
        let fixture = Fixture::new(pool).await;
        fixture.save(update("Studio")).await.unwrap();
        let gallery = fixture.gallery(true).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;
        let stamp = fixture.proof(photo).await.unwrap().stamp;
        let mut transaction = fixture.pool.begin().await.unwrap();
        sqlx::raw_sql("ALTER TABLE workspace_settings DROP COLUMN studio_mode, DROP COLUMN studio_palette; DROP TYPE studio_mode, studio_palette")
            .execute(&mut *transaction).await.unwrap();
        sqlx::raw_sql(include_str!("../migrations/0044_studio_display.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();
        let stored: (String,String,String) = sqlx::query_as("SELECT studio_name, studio_mode::text, studio_palette::text FROM workspace_settings WHERE workspace_id=$1")
            .bind(fixture.workspace_id).fetch_one(&mut *transaction).await.unwrap();
        assert_eq!(
            stored,
            ("Studio".into(), "full_image".into(), "warm".into())
        );
        transaction.rollback().await.unwrap();

        fixture
            .watermarks()
            .save_display(
                fixture.workspace_id,
                StudioMode::Print,
                StudioPalette::Neutral,
            )
            .await
            .unwrap();
        assert_eq!(fixture.proof(photo).await.unwrap().stamp, stamp);
        assert_eq!(fixture.jobs(photo).await, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn signature_color_migration_keeps_saved_settings_and_inherits_cover_color(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.save(update("Studio")).await.unwrap();

        let mut transaction = fixture.pool.begin().await.unwrap();

        sqlx::raw_sql("ALTER TABLE workspace_settings DROP COLUMN signature_color")
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0043_signature_color.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();

        let settings: (String, Option<WatermarkColor>) =
            sqlx::query_as("SELECT studio_name, signature_color FROM workspace_settings")
                .fetch_one(&mut *transaction)
                .await
                .unwrap();

        assert_eq!(settings, ("Studio".to_owned(), None));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn ten_size_steps_keep_saved_watermarks_as_large_as_before(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture
            .save(SettingsUpdate {
                size: SizeStep::constant::<3>(),
                ..update("Studio")
            })
            .await
            .unwrap();

        let mut transaction = fixture.pool.begin().await.unwrap();

        sqlx::raw_sql(
            "ALTER TABLE workspace_settings DROP COLUMN signature_size,
                 DROP CONSTRAINT workspace_settings_size_range;
             ALTER TABLE workspace_settings ADD CONSTRAINT workspace_settings_size_range
                 CHECK (size BETWEEN 0 AND 4);",
        )
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0027_ten_size_steps.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();

        let (size, signature_size): (i16, i16) =
            sqlx::query_as("SELECT size, signature_size FROM workspace_settings")
                .fetch_one(&mut *transaction)
                .await
                .unwrap();

        assert_eq!((size, signature_size), (6, 4));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn upgrade_requires_new_clean_publication_and_preserves_finals(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let proof = fixture.photo(gallery, PhotoStatus::Ready).await;
        let finished = fixture
            .photo_of_kind(gallery, PhotoKind::Final, PhotoStatus::Ready)
            .await;
        let mut transaction = fixture.pool.begin().await.unwrap();

        sqlx::raw_sql("ALTER TABLE photos DROP COLUMN clean_ready, DROP COLUMN watermark_revision")
            .execute(&mut *transaction)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("../migrations/0017_proof_variants.sql"))
            .execute(&mut *transaction)
            .await
            .unwrap();
        let state: (bool, Option<i64>, String) = sqlx::query_as(
            "SELECT clean_ready, watermark_revision, status::text FROM photos WHERE id = $1",
        )
        .bind(proof)
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
        let final_status: String =
            sqlx::query_scalar("SELECT status::text FROM photos WHERE id = $1")
                .bind(finished)
                .fetch_one(&mut *transaction)
                .await
                .unwrap();
        let queued: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM jobs WHERE payload->'payload'->>'photo_id' = $1",
        )
        .bind(proof.to_string())
        .fetch_one(&mut *transaction)
        .await
        .unwrap();

        assert_eq!(state, (false, None, "waiting".to_owned()));
        assert_eq!(final_status, "ready");
        assert_eq!(queued, 1);

        transaction.rollback().await.unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn clean_access_survives_marked_failure_and_toggle(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(false).await;
        let photo = fixture.photo(gallery, PhotoStatus::Waiting).await;
        let photos = fixture.database.repository::<PhotoRepository>();

        photos.mark_ready(photo).await.unwrap();
        let location = photos.locate(photo).await.unwrap().unwrap();

        fixture.save(update("Studio")).await.unwrap();
        photos.mark_failed(photo, "Rendering failed").await.unwrap();

        assert!(
            photos
                .is_ready(location, photo, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap()
        );
        assert!(
            !photos
                .is_ready(location, photo, PhotoKind::Proof, Watermarked(true))
                .await
                .unwrap()
        );
        assert_eq!(
            photos
                .set_favorite(location, photo, true, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap(),
            FavoriteChange::Applied
        );

        fixture
            .watermarks()
            .update_gallery(
                fixture.workspace_id,
                gallery,
                &GalleryChanges {
                    watermark_enabled: Some(true),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap();

        assert!(
            !photos
                .is_ready(location, photo, PhotoKind::Proof, Watermarked(true))
                .await
                .unwrap()
        );
        assert!(
            photos
                .is_ready(location, photo, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap()
        );
        assert!(
            photos
                .list_ready(location, PhotoKind::Proof, Watermarked(true))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            photos
                .list_ready(location, PhotoKind::Proof, Watermarked(false))
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            photos
                .set_favorite(location, photo, false, PhotoKind::Proof, Watermarked(true))
                .await
                .unwrap(),
            FavoriteChange::NotFound
        );
        assert_eq!(
            photos.favorites(location, PhotoKind::Proof).await.unwrap(),
            vec![photo]
        );
        assert_eq!(fixture.jobs(photo).await, 1);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn toggle_does_not_reset_or_enqueue(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(false).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;

        for enabled in [true, false] {
            fixture
                .watermarks()
                .update_gallery(
                    fixture.workspace_id,
                    gallery,
                    &GalleryChanges {
                        watermark_enabled: Some(enabled),
                        ..GalleryChanges::default()
                    },
                )
                .await
                .unwrap();

            assert_eq!(fixture.status(photo).await, PhotoStatus::Ready);
            assert_eq!(fixture.jobs(photo).await, 0);
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn settings_update_requeues_disabled_and_enabled_proofs(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        for enabled in [false, true] {
            let gallery = fixture.gallery(enabled).await;
            let photo = fixture.photo(gallery, PhotoStatus::Ready).await;

            fixture
                .save(update(if enabled { "Second" } else { "First" }))
                .await
                .unwrap();

            assert_eq!(fixture.status(photo).await, PhotoStatus::Waiting);
            assert_eq!(fixture.jobs(photo).await, 1);
        }
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_waiting_upload_cannot_start_with_settings_that_skip_its_regeneration(pool: PgPool) {
        waiting_upload_during_settings_save(pool, true).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn regeneration_requeues_an_upload_that_starts_before_its_row_is_locked(pool: PgPool) {
        waiting_upload_during_settings_save(pool, false).await;
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn an_upload_inserted_during_settings_save_retains_a_current_job(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.save(update("Before")).await.unwrap();

        let gallery = fixture.gallery(false).await;
        let blocker = fixture.photo(gallery, PhotoStatus::Ready).await;
        let mut locked = fixture.pool.begin().await.unwrap();

        sqlx::query("SELECT id FROM photos WHERE id = $1 FOR UPDATE")
            .bind(blocker)
            .fetch_one(&mut *locked)
            .await
            .unwrap();

        let saving = task::spawn({
            let database = fixture.database.clone();
            let workspace = fixture.workspace_id;

            async move {
                database
                    .repository::<SettingsRepository>()
                    .save(workspace, update("After"))
                    .await
                    .unwrap();
            }
        });

        time::timeout(
            Duration::from_secs(5),
            fixture
                .database
                .repository::<TestRepository>()
                .wait_for_a_lock_waiter(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut inserting = task::spawn({
            let fixture = Fixture {
                pool: fixture.pool.clone(),
                database: fixture.database.clone(),
                workspace_id: fixture.workspace_id,
            };

            async move { fixture.photo(gallery, PhotoStatus::Waiting).await }
        });
        let inserted = tokio::select! {
            upload = &mut inserting => Some(upload.unwrap()),
            () = wait_for_lock_waiters(&fixture.pool, 2) => None,
        };

        locked.commit().await.unwrap();
        saving.await.unwrap();

        assert!(
            inserted.is_none(),
            "new uploads must wait for the settings snapshot"
        );

        let upload = inserting.await.unwrap();
        let jobs = fixture.database.repository::<JobRepository>();
        let blocker_job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        assert_eq!(
            blocker_job.payload::<ProcessPhoto>().unwrap().photo_id,
            blocker
        );
        assert!(jobs.complete(&blocker_job).await.unwrap().is_current());

        jobs.enqueue(ProcessPhoto { photo_id: upload }, 5.try_into().unwrap())
            .await
            .unwrap();

        let job = claimed_job(&fixture, upload).await;

        assert!(
            fixture
                .database
                .repository::<PhotoRepository>()
                .mark_processing(upload)
                .await
                .unwrap()
        );

        let proof = fixture.proof(upload).await.unwrap();

        assert_eq!(proof.stamp.revision(), Some(2));

        let Publication::Open(publication) = jobs
            .begin_publication(&job, upload, proof.stamp)
            .await
            .unwrap()
        else {
            panic!("the newly accepted upload must retain a publishable job");
        };
        let (completion, photo_update) = publication.finish().await.unwrap();

        assert!(completion.is_current());
        assert_eq!(photo_update, PhotoUpdate::Saved);
        assert_eq!(fixture.jobs(upload).await, 0);
        assert_eq!(fixture.status(upload).await, PhotoStatus::Ready);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_failed_retry_during_settings_save_keeps_a_publishable_job(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture.save(update("Before")).await.unwrap();

        let gallery = fixture.gallery(false).await;
        let failed = fixture
            .photo(gallery, PhotoStatus::Failed("Processing failed".to_owned()))
            .await;
        let blocker = fixture.photo(gallery, PhotoStatus::Ready).await;
        let mut locked = fixture.pool.begin().await.unwrap();

        sqlx::query("SELECT id FROM photos WHERE id = $1 FOR UPDATE")
            .bind(blocker)
            .fetch_one(&mut *locked)
            .await
            .unwrap();

        let saving = task::spawn({
            let database = fixture.database.clone();
            let workspace = fixture.workspace_id;

            async move {
                database
                    .repository::<SettingsRepository>()
                    .save(workspace, update("After"))
                    .await
                    .unwrap();
            }
        });

        wait_for_lock_waiters(&fixture.pool, 1).await;

        let mut retrying = task::spawn({
            let database = fixture.database.clone();
            let workspace = fixture.workspace_id;

            async move {
                assert_eq!(
                    database
                        .repository::<PhotoRetry>()
                        .retry_failed(workspace, gallery, PhotoKind::Proof, Some(failed))
                        .await
                        .unwrap(),
                    Some(1)
                );
                assert!(
                    database
                        .repository::<PhotoRepository>()
                        .mark_processing(failed)
                        .await
                        .unwrap()
                );

                database
                    .repository::<SettingsRepository>()
                    .proof_watermark(failed)
                    .await
                    .unwrap()
                    .unwrap()
            }
        });
        let proof = tokio::select! {
            proof = &mut retrying => Some(proof.unwrap()),
            () = wait_for_lock_waiters(&fixture.pool, 2) => None,
        };

        locked.commit().await.unwrap();
        saving.await.unwrap();

        let proof = match proof {
            Some(proof) => proof,
            None => retrying.await.unwrap(),
        };

        assert_eq!(proof.stamp.revision(), Some(2));
        assert_eq!(fixture.jobs(failed).await, 1);

        let jobs = fixture.database.repository::<JobRepository>();
        let blocker_job = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();

        assert_eq!(
            blocker_job.payload::<ProcessPhoto>().unwrap().photo_id,
            blocker
        );
        assert!(jobs.complete(&blocker_job).await.unwrap().is_current());

        let job = claimed_job(&fixture, failed).await;
        let Publication::Open(publication) = jobs
            .begin_publication(&job, failed, proof.stamp)
            .await
            .unwrap()
        else {
            panic!("the manual retry must retain a publishable job");
        };
        let (completion, photo_update) = publication.finish().await.unwrap();

        assert!(completion.is_current());
        assert_eq!(photo_update, PhotoUpdate::Saved);
        assert_eq!(fixture.status(failed).await, PhotoStatus::Ready);
        assert_eq!(fixture.jobs(failed).await, 0);
    }

    async fn waiting_upload_during_settings_save(pool: PgPool, upload_before_blocker: bool) {
        let fixture = Fixture::new(pool).await;

        fixture.save(update("Before")).await.unwrap();

        let gallery = fixture.gallery(false).await;
        fixture.photo(gallery, PhotoStatus::Ready).await;
        fixture.photo(gallery, PhotoStatus::Ready).await;

        let ids: Vec<PhotoId> =
            sqlx::query_scalar("SELECT id FROM photos WHERE workspace_id = $1 ORDER BY id")
                .bind(fixture.workspace_id)
                .fetch_all(&fixture.pool)
                .await
                .unwrap();
        let (upload, blocker) = if upload_before_blocker {
            (ids[0], ids[1])
        } else {
            (ids[1], ids[0])
        };
        let jobs = fixture.database.repository::<JobRepository>();

        fixture
            .database
            .repository::<TestRepository>()
            .force_photo_status(upload, &PhotoStatus::Waiting)
            .await
            .unwrap();
        jobs.enqueue(ProcessPhoto { photo_id: upload }, 5.try_into().unwrap())
            .await
            .unwrap();

        let job = claimed_job(&fixture, upload).await;
        let mut locked = fixture.pool.begin().await.unwrap();

        sqlx::query("SELECT id FROM photos WHERE id = $1 FOR UPDATE")
            .bind(blocker)
            .fetch_one(&mut *locked)
            .await
            .unwrap();

        let saving = task::spawn({
            let database = fixture.database.clone();
            let workspace = fixture.workspace_id;

            async move {
                database
                    .repository::<SettingsRepository>()
                    .save(workspace, update("After"))
                    .await
                    .unwrap();
            }
        });

        time::timeout(
            Duration::from_secs(5),
            fixture
                .database
                .repository::<TestRepository>()
                .wait_for_a_lock_waiter(),
        )
        .await
        .unwrap()
        .unwrap();

        // Pause the save during its ordered scan. The upload must either wait for the save
        // or retain a replacement job when it starts before the scan reaches its row.
        let mut starting = task::spawn({
            let database = fixture.database.clone();

            async move {
                assert!(
                    database
                        .repository::<PhotoRepository>()
                        .mark_processing(upload)
                        .await
                        .unwrap()
                );
                database
                    .repository::<SettingsRepository>()
                    .proof_watermark(upload)
                    .await
                    .unwrap()
                    .unwrap()
            }
        });
        let proof = tokio::select! {
            proof = &mut starting => Some(proof.unwrap()),
            () = wait_for_lock_waiters(&fixture.pool, 2) => None,
        };

        locked.commit().await.unwrap();
        saving.await.unwrap();

        let proof = match proof {
            Some(proof) => proof,
            None => starting.await.unwrap(),
        };

        if !upload_before_blocker {
            assert_eq!(proof.stamp.revision(), Some(1));
            assert_eq!(fixture.status(upload).await, PhotoStatus::Waiting);
            assert_eq!(fixture.jobs(upload).await, 2);

            assert!(matches!(
                jobs.begin_publication(&job, upload, proof.stamp)
                    .await
                    .unwrap(),
                Publication::Closed(_, PhotoUpdate::Unchanged)
            ));
            assert_eq!(fixture.jobs(upload).await, 1);

            return;
        }

        assert_eq!(proof.stamp.revision(), Some(2));
        assert_eq!(fixture.jobs(upload).await, 1);

        let Publication::Open(publication) = jobs
            .begin_publication(&job, upload, proof.stamp)
            .await
            .unwrap()
        else {
            panic!("the waiting upload must retain a publishable job");
        };
        let (completion, photo_update) = publication.finish().await.unwrap();

        assert!(completion.is_current());
        assert_eq!(photo_update, PhotoUpdate::Saved);
        assert_eq!(fixture.status(upload).await, PhotoStatus::Ready);
        assert_eq!(fixture.jobs(upload).await, 0);
    }

    async fn wait_for_lock_waiters(pool: &PgPool, count: i64) {
        time::timeout(Duration::from_secs(5), async {
            loop {
                let waiters: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM pg_stat_activity
                     WHERE datname = current_database() AND wait_event_type = 'Lock'",
                )
                .fetch_one(pool)
                .await
                .unwrap();

                if waiters >= count {
                    return;
                }

                time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn flipping_a_gallery_switch_leaves_all_photos_alone(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(false).await;
        let other = fixture.gallery(false).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;
        let bystander = fixture.photo(other, PhotoStatus::Ready).await;
        let flip = |enabled| GalleryChanges {
            watermark_enabled: Some(enabled),
            ..GalleryChanges::default()
        };
        let retitle = GalleryChanges {
            title: Some(Some("Wedding".to_owned())),
            ..GalleryChanges::default()
        };

        for (changes, expected_jobs) in [(retitle, 0), (flip(false), 0), (flip(true), 0)] {
            fixture
                .watermarks()
                .update_gallery(fixture.workspace_id, gallery, &changes)
                .await
                .unwrap()
                .unwrap();

            assert_eq!(fixture.jobs(photo).await, expected_jobs);
        }

        assert_eq!(fixture.status(photo).await, PhotoStatus::Ready);
        assert_eq!(fixture.status(bystander).await, PhotoStatus::Ready);

        // Switching off selects existing clean files without processing.
        fixture
            .database
            .repository::<TestRepository>()
            .force_photo_status(photo, &PhotoStatus::Ready)
            .await
            .unwrap();
        fixture
            .watermarks()
            .update_gallery(fixture.workspace_id, gallery, &flip(false))
            .await
            .unwrap();

        assert_eq!(fixture.jobs(photo).await, 0);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn updating_a_missing_gallery_finds_nothing(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        assert!(
            fixture
                .watermarks()
                .update_gallery(
                    fixture.workspace_id,
                    GalleryId::generate(),
                    &GalleryChanges {
                        watermark_enabled: Some(true),
                        ..GalleryChanges::default()
                    },
                )
                .await
                .unwrap()
                .is_none()
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_proof_watermark_names_what_to_draw_and_its_stamp(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let off = fixture.gallery(false).await;
        let on = fixture.gallery(true).await;
        let plain = fixture.photo(off, PhotoStatus::Waiting).await;
        let marked = fixture.photo(on, PhotoStatus::Waiting).await;
        let first = fixture.proof(marked).await.unwrap();

        assert_eq!(first.watermark, None);
        assert_eq!(
            fixture.proof(plain).await.unwrap().stamp,
            WatermarkStamp::new(Some(0))
        );
        assert_eq!(first.stamp, WatermarkStamp::new(Some(0)));
        assert!(fixture.proof(PhotoId::generate()).await.is_none());

        fixture.save(update("Studio")).await.unwrap();

        let second = fixture.proof(marked).await.unwrap();

        assert_eq!(second.stamp, WatermarkStamp::new(Some(1)));
        assert_eq!(second.watermark.unwrap().text.unwrap().value, "Studio");
        assert_eq!(
            fixture
                .proof(plain)
                .await
                .unwrap()
                .watermark
                .unwrap()
                .text
                .unwrap()
                .value,
            "Studio"
        );
    }

    /// Claims the only queued job of `photo`'s gallery after making it waiting.
    async fn claimed_job(fixture: &Fixture, photo: PhotoId) -> Job {
        let jobs = fixture.database.repository::<JobRepository>();

        assert!(fixture.jobs(photo).await == 1);

        jobs.claim(KINDS, LEASE).await.unwrap().unwrap()
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_current_render_publishes_and_finishes_the_job(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;

        fixture.save(update("Studio")).await.unwrap();

        let job = claimed_job(&fixture, photo).await;
        let proof = fixture
            .watermarks()
            .proof_watermark(photo)
            .await
            .unwrap()
            .unwrap();
        let jobs = fixture.database.repository::<JobRepository>();
        fixture
            .watermarks()
            .update_gallery(
                fixture.workspace_id,
                gallery,
                &GalleryChanges {
                    watermark_enabled: Some(false),
                    ..GalleryChanges::default()
                },
            )
            .await
            .unwrap();
        let Publication::Open(open) = jobs
            .begin_publication(&job, photo, proof.stamp)
            .await
            .unwrap()
        else {
            panic!("a current render must publish");
        };
        let (completion, update) = open.finish().await.unwrap();

        assert!(completion.is_current());
        assert_eq!(update, PhotoUpdate::Saved);
        assert_eq!(fixture.status(photo).await, PhotoStatus::Ready);
        assert_eq!(
            fixture
                .database
                .repository::<TestRepository>()
                .count_jobs(None)
                .await
                .unwrap(),
            0
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_render_made_before_a_change_is_not_published(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;

        fixture.save(update("Studio")).await.unwrap();

        let job = claimed_job(&fixture, photo).await;
        let proof = fixture
            .watermarks()
            .proof_watermark(photo)
            .await
            .unwrap()
            .unwrap();

        fixture
            .database
            .repository::<PhotoRepository>()
            .mark_processing(photo)
            .await
            .unwrap();
        fixture.save(update("Other name")).await.unwrap();

        let jobs = fixture.database.repository::<JobRepository>();
        let Publication::Closed(completion, photo_update) = jobs
            .begin_publication(&job, photo, proof.stamp)
            .await
            .unwrap()
        else {
            panic!("a stale render must not publish");
        };

        assert!(completion.is_current());
        assert_eq!(photo_update, PhotoUpdate::Unchanged);
        assert_eq!(fixture.status(photo).await, PhotoStatus::Waiting);
        assert_eq!(fixture.jobs(photo).await, 1);

        let next = jobs.claim(KINDS, LEASE).await.unwrap().unwrap();
        let fresh = fixture
            .watermarks()
            .proof_watermark(photo)
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(
            jobs.begin_publication(&next, photo, fresh.stamp)
                .await
                .unwrap(),
            Publication::Open(_)
        ));
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn a_lost_claim_or_deleted_photo_publishes_nothing(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let gallery = fixture.gallery(true).await;
        let photo = fixture.photo(gallery, PhotoStatus::Ready).await;

        fixture.save(update("Studio")).await.unwrap();

        let job = claimed_job(&fixture, photo).await;
        let stamp = fixture
            .watermarks()
            .proof_watermark(photo)
            .await
            .unwrap()
            .unwrap()
            .stamp;
        let jobs = fixture.database.repository::<JobRepository>();

        // The same job finished by someone else.
        assert!(jobs.complete(&job).await.unwrap().is_current());

        let Publication::Closed(completion, photo_update) =
            jobs.begin_publication(&job, photo, stamp).await.unwrap()
        else {
            panic!("a lost claim must not publish");
        };

        assert!(!completion.is_current());
        assert_eq!(photo_update, PhotoUpdate::Unchanged);

        sqlx::query("DELETE FROM photos WHERE id = $1")
            .bind(photo)
            .execute(&fixture.pool)
            .await
            .unwrap();

        let Publication::Closed(_, photo_update) =
            jobs.begin_publication(&job, photo, stamp).await.unwrap()
        else {
            panic!("a deleted photo must not publish");
        };

        assert_eq!(photo_update, PhotoUpdate::Missing);
    }
}
