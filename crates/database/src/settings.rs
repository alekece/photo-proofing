//! Workspace watermark settings and preparation of marked proof copies.
//!
//! The *watermark* is what is actually drawn on a proof: [`Watermark`]. Changing the effective
//! watermark, or the proof quality, requeues proofs in every gallery. A gallery switch only
//! chooses the served pair. Clean readiness survives marked-only regeneration, while client
//! marked access requires the current workspace revision.
//! A render proves it used the current settings with a [`WatermarkStamp`]; see
//! [`JobRepository::begin_publication`](JobRepository::begin_publication).

use std::num::NonZeroU16;

use serde::{Deserialize, Serialize};
use sqlx::{
    AssertSqlSafe, FromRow, PgConnection, PgPool, Postgres, Row, Transaction, Type, postgres::PgRow,
};
use thiserror::Error;

use crate::photo::PhotoId;
use crate::{
    Repository,
    auth::WorkspaceId,
    error::DatabaseError,
    gallery::{Gallery, GalleryChanges, GalleryError, GalleryId, GalleryRepository},
    jobs::JobOptions,
};

/// Longest studio name, in characters.
pub const STUDIO_NAME_MAX_CHARS: usize = 100;

/// Why settings were refused.
#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("the studio name is required when the watermark shows it")]
    NameRequired,
    #[error("the studio name is longer than {STUDIO_NAME_MAX_CHARS} characters")]
    NameTooLong,
    #[error("a logo is required when the watermark or cover signature shows it")]
    LogoRequired,
    #[error("grid cell {0} is out of range")]
    GridCell(u8),
    #[error("size step {0} is out of range")]
    SizeStep(u8),
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

/// How the studio signs the client cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "cover_signature", rename_all = "snake_case")]
pub enum CoverSignature {
    Name,
    Logo,
    Both,
    None,
}

impl CoverSignature {
    const fn shows_logo(self) -> bool {
        matches!(self, Self::Logo | Self::Both)
    }
}

/// The language of the photographer app and every client page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "ui_language", rename_all = "snake_case")]
pub enum Language {
    En,
    Fr,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct GridCell(u8);

impl GridCell {
    pub const BOTTOM_RIGHT: Self = Self(8);

    /// The cell's index, 0 to 8.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for GridCell {
    type Error = SettingsError;

    fn try_from(cell: u8) -> Result<Self, Self::Error> {
        if cell > Self::BOTTOM_RIGHT.0 {
            return Err(SettingsError::GridCell(cell));
        }

        Ok(Self(cell))
    }
}

impl From<GridCell> for u8 {
    fn from(cell: GridCell) -> Self {
        cell.0
    }
}

/// How large the watermark or the cover signature is: 0 (small) to 9 (large).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct SizeStep(u8);

impl SizeStep {
    pub const LARGEST: Self = Self(9);

    /// The step, 0 to 9.
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for SizeStep {
    /// The step that matches the original fixed look.
    fn default() -> Self {
        Self(4)
    }
}

impl TryFrom<u8> for SizeStep {
    type Error = SettingsError;

    fn try_from(step: u8) -> Result<Self, Self::Error> {
        if step > Self::LARGEST.0 {
            return Err(SettingsError::SizeStep(step));
        }

        Ok(Self(step))
    }
}

impl From<SizeStep> for u8 {
    fn from(step: SizeStep) -> Self {
        step.0
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
    pub text: Option<String>,
    /// PNG bytes.
    pub logo: Option<Vec<u8>>,
    pub placement: WatermarkPlacement,
    pub size: SizeStep,
}

/// The saved workspace settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSettings {
    pub studio_name: String,
    pub mark: WatermarkMark,
    pub pattern: WatermarkPattern,
    pub position: GridCell,
    pub size: SizeStep,
    pub signature: CoverSignature,
    pub signature_size: SizeStep,
    pub proof_quality: ProofQuality,
    /// Normalised PNG bytes.
    pub logo: Option<Vec<u8>>,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            studio_name: String::new(),
            mark: WatermarkMark::Name,
            pattern: WatermarkPattern::Repeat,
            position: GridCell::BOTTOM_RIGHT,
            size: SizeStep::default(),
            signature: CoverSignature::Name,
            signature_size: SizeStep::default(),
            proof_quality: ProofQuality::default(),
            logo: None,
        }
    }
}

impl WorkspaceSettings {
    /// What these settings draw, or `None` while there is nothing to draw yet.
    pub fn watermark(&self) -> Option<Watermark> {
        let text = Some(self.studio_name.clone())
            .filter(|name| self.mark.shows_name() && !name.is_empty());
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
        })
    }
}

impl FromRow<'_, PgRow> for WorkspaceSettings {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let narrow = |column: &str| {
            u8::try_from(row.try_get::<i16, _>(column)?)
                .map_err(|error| sqlx::Error::Decode(error.into()))
        };

        Ok(Self {
            studio_name: row.try_get("studio_name")?,
            mark: row.try_get("mark")?,
            pattern: row.try_get("pattern")?,
            position: GridCell::try_from(narrow("position")?)
                .map_err(|error| sqlx::Error::Decode(error.into()))?,
            size: SizeStep::try_from(narrow("size")?)
                .map_err(|error| sqlx::Error::Decode(error.into()))?,
            signature: row.try_get("signature")?,
            signature_size: SizeStep::try_from(narrow("signature_size")?)
                .map_err(|error| sqlx::Error::Decode(error.into()))?,
            proof_quality: row.try_get("proof_quality")?,
            logo: row.try_get("logo")?,
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
    pub studio_name: String,
    pub mark: WatermarkMark,
    pub pattern: WatermarkPattern,
    pub position: GridCell,
    pub size: SizeStep,
    pub signature: CoverSignature,
    pub signature_size: SizeStep,
    pub proof_quality: ProofQuality,
    pub logo: LogoChange,
}

/// Identifies the workspace watermark a render used, independently of the gallery switch.
/// Finals have no watermark revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatermarkStamp(Option<i64>);

impl WatermarkStamp {
    /// SQL for the stamp of the photo `p`, which only proofs have; needs `galleries g` and `workspace_settings s` joined.
    pub(crate) const SQL: &'static str = "CASE WHEN p.kind = 'proof'
         THEN COALESCE(s.watermark_revision, 0) END";

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
    /// Whether the clean files exist at that quality, so only the marked ones need rendering.
    pub clean_current: bool,
    /// `None` for finals or when nothing is saved to draw.
    pub watermark: Option<Watermark>,
}

/// Data access for the workspace settings; see the module docs.
pub struct SettingsRepository<'db> {
    pool: &'db PgPool,
    jobs: JobOptions,
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
        COALESCE(s.mark, 'name') AS mark, COALESCE(s.pattern, 'repeat') AS pattern,
        COALESCE(s.position, 8::smallint) AS position, COALESCE(s.size, 4::smallint) AS size,
        COALESCE(s.signature, 'name') AS signature,
        COALESCE(s.signature_size, 4::smallint) AS signature_size,
        COALESCE(s.proof_quality, 'medium') AS proof_quality, s.logo AS logo";

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

    /// Saves the settings, and regenerates the proofs of every gallery when what is
    /// drawn changed. All in one transaction.
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

        // Photo insertion takes a foreign-key lock on this row. Prevent new uploads from
        // escaping the regeneration snapshot and starting against uncommitted settings.
        sqlx::query("SELECT id FROM workspaces WHERE id = $1 FOR UPDATE")
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
            "SELECT studio_name, mark, pattern, position, size, signature, signature_size,
                    proof_quality, logo
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
            mark: update.mark,
            pattern: update.pattern,
            position: update.position,
            size: update.size,
            signature: update.signature,
            signature_size: update.signature_size,
            proof_quality: update.proof_quality,
            logo,
        };
        let changed = saved.watermark() != current.watermark()
            || saved.proof_quality != current.proof_quality;

        sqlx::query(
            "UPDATE workspace_settings SET
                 studio_name = $2, mark = $3, pattern = $4, position = $5, size = $6,
                 signature = $7, logo = $8, proof_quality = $10, signature_size = $11,
                 watermark_revision = watermark_revision + $9, updated_at = now()
             WHERE workspace_id = $1",
        )
        .bind(workspace_id)
        .bind(&saved.studio_name)
        .bind(saved.mark)
        .bind(saved.pattern)
        .bind(i16::from(saved.position.get()))
        .bind(i16::from(saved.size.get()))
        .bind(saved.signature)
        .bind(&saved.logo)
        .bind(i64::from(changed))
        .bind(saved.proof_quality)
        .bind(i16::from(saved.signature_size.get()))
        .execute(&mut *transaction)
        .await?;

        if changed {
            Self::regenerate_in(&mut transaction, workspace_id, self.jobs.max_attempts).await?;
        }

        transaction.commit().await?;

        Ok(saved)
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
        GalleryRepository::open(self.pool, ())
            .update(workspace_id, id, changes)
            .await
    }

    /// What `photo`'s proofs must show right now, or `None` if the photo does not exist.
    ///
    /// Read once, so the stamp always describes the watermark returned with it.
    pub async fn proof_watermark(
        &self,
        photo: PhotoId,
    ) -> Result<Option<ProofWatermark>, DatabaseError> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT p.kind = 'proof' AS enabled,
                    p.clean_ready AND p.clean_quality = COALESCE(s.proof_quality, 'medium')
                        AS clean_current,
                    {stamp} AS stamp, {settings}
             FROM photos p
             JOIN galleries g ON g.id = p.gallery_id
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
                watermark,
            })
        })
        .transpose()
        .map_err(|error: sqlx::Error| error.into())
    }

    /// Sends the workspace's `ready` and `processing` proofs back to `waiting` with a new job
    /// each. Waiting proofs also lock, so their workers cannot read the old settings after
    /// being skipped by regeneration. Photos lock in id order; jobs are only inserted, so
    /// this cannot deadlock with a worker, which locks a job and then its photo.
    async fn regenerate_in(
        transaction: &mut Transaction<'_, Postgres>,
        workspace_id: WorkspaceId,
        max_attempts: NonZeroU16,
    ) -> Result<u64, sqlx::Error> {
        let connection: &mut PgConnection = transaction;
        let queued = sqlx::query(
            "WITH locked AS (
                 SELECT p.id, p.status FROM photos p JOIN galleries g ON g.id = p.gallery_id
                 WHERE p.workspace_id = $1 AND p.kind = 'proof'
                   AND p.status IN ('waiting', 'ready', 'processing')
                 ORDER BY p.id
                 FOR UPDATE OF p
             ), reset AS (
                 UPDATE photos SET status = 'waiting', failure_reason = NULL
                 WHERE id IN (SELECT id FROM locked WHERE status IN ('ready', 'processing'))
                 RETURNING id
             )
             INSERT INTO jobs(id, payload, max_attempts)
             SELECT gen_random_uuid(),
                    jsonb_build_object('kind', 'process_photo',
                                       'payload', jsonb_build_object('photo_id', id)),
                    $2
             FROM reset",
        )
        .bind(workspace_id)
        .bind(i32::from(max_attempts.get()))
        .execute(connection)
        .await?;

        Ok(queued.rows_affected())
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
        jobs::{Job, JobKind, JobRepository, PhotoUpdate, ProcessPhoto, Publication},
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
            mark: WatermarkMark::Name,
            pattern: WatermarkPattern::Repeat,
            position: GridCell::BOTTOM_RIGHT,
            size: SizeStep(2),
            signature: CoverSignature::Name,
            signature_size: SizeStep::default(),
            proof_quality: ProofQuality::Medium,
            logo: LogoChange::Keep,
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
                size: SizeStep(4),
                signature: CoverSignature::None,
                signature_size: SizeStep(7),
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
                text: Some("Studio Lumière".to_owned()),
                logo: Some(PNG.to_vec()),
                placement: WatermarkPlacement::Once(GridCell::try_from(4).unwrap()),
                size: SizeStep(4),
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
    async fn a_changed_watermark_regenerates_all_galleries(pool: PgPool) {
        let fixture = Fixture::new(pool).await;
        let on = fixture.gallery(true).await;
        let off = fixture.gallery(false).await;
        let ready = fixture.photo(on, PhotoStatus::Ready).await;
        let processing = fixture.photo(on, PhotoStatus::Processing).await;
        let failed = fixture
            .photo(on, PhotoStatus::Failed("bad".to_owned()))
            .await;
        let unmarked = fixture.photo(off, PhotoStatus::Ready).await;

        fixture.save(update("Studio")).await.unwrap();

        for id in [ready, processing] {
            assert_eq!(fixture.status(id).await, PhotoStatus::Waiting);
            assert_eq!(fixture.jobs(id).await, 1);
        }

        assert_eq!(
            fixture.status(failed).await,
            PhotoStatus::Failed("bad".to_owned())
        );
        assert_eq!(fixture.jobs(failed).await, 0);
        assert_eq!(fixture.status(unmarked).await, PhotoStatus::Waiting);
        assert_eq!(fixture.jobs(unmarked).await, 1);
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

        for id in [marked, unmarked] {
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

        // The cover signature and its size, an unused logo and the position of a repeated mark are not drawn.
        fixture
            .save(SettingsUpdate {
                signature: CoverSignature::None,
                signature_size: SizeStep(9),
                position: GridCell::try_from(0).unwrap(),
                logo: LogoChange::Replace(PNG.to_vec()),
                ..update("Studio")
            })
            .await
            .unwrap();

        assert_eq!(fixture.status(photo).await, PhotoStatus::Ready);
        assert_eq!(fixture.jobs(photo).await, 0);

        fixture
            .save(SettingsUpdate {
                size: SizeStep(3),
                ..update("Studio")
            })
            .await
            .unwrap();

        assert_eq!(fixture.status(photo).await, PhotoStatus::Waiting);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn ten_size_steps_keep_saved_watermarks_as_large_as_before(pool: PgPool) {
        let fixture = Fixture::new(pool).await;

        fixture
            .save(SettingsUpdate {
                size: SizeStep(3),
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
        sqlx::raw_sql(include_str!("../migrations/0026_ten_size_steps.sql"))
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
        assert_eq!(second.watermark.unwrap().text.as_deref(), Some("Studio"));
        assert_eq!(
            fixture
                .proof(plain)
                .await
                .unwrap()
                .watermark
                .unwrap()
                .text
                .as_deref(),
            Some("Studio")
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
