//! The private link token of a gallery and the client's view of a shared gallery.

use std::num::NonZeroUsize;

use derive_more::{Deref, FromStr};
use pp_core::types::{timestamp::Timestamp, token::Token};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, Type, postgres::PgRow};
use time::Date;

use super::{
    CoverLayout, EncryptedAccessCode, FocalPoint, Gallery, GalleryLayout, GridDensity, GridSpacing,
    HeadingFont,
};
use crate::{
    contact::ContactId,
    photo::{PhotoId, PhotoLocation},
};

/// The unguessable value that makes up a gallery's private link.
///
/// 128 bits is out of reach of online guessing and keeps the link short enough to paste
/// anywhere. It is stored as is, not hashed, because sharing again must show the same link.
#[derive(Debug, Clone, PartialEq, Eq, Deref, FromStr, Serialize, Deserialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct ShareToken(Token<16>);

impl ShareToken {
    pub fn generate() -> Self {
        Self(Token::generate())
    }
}

/// What a client holding the link may see of a gallery, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SharedGallery {
    /// `None` means the gallery has no title yet.
    pub title: Option<String>,
    pub layout: GalleryLayout,
    pub cover_layout: CoverLayout,
    pub focal_point: FocalPoint,
    pub heading_font: HeadingFont,
    pub grid_density: GridDensity,
    pub grid_spacing: GridSpacing,
    /// The day of the shoot; `None` means none was given.
    pub shoot_date: Option<Date>,
    /// `None` means the gallery never expires.
    pub expires_at: Option<Timestamp>,
    /// How many photos the client may pick before extra ones are charged; `None` means selection
    /// is unlimited.
    pub included_photos: Option<NonZeroUsize>,
    /// Once delivered, the selection is final.
    pub delivered: bool,
    /// The gallery's hero photo, only while it is ready.
    pub hero_id: Option<PhotoId>,
}

impl FromRow<'_, PgRow> for SharedGallery {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            title: row.try_get("title")?,
            layout: row.try_get("layout")?,
            cover_layout: row.try_get("cover_layout")?,
            focal_point: FocalPoint::from_row(row)?,
            heading_font: row.try_get("heading_font")?,
            grid_density: row.try_get("grid_density")?,
            grid_spacing: row.try_get("grid_spacing")?,
            shoot_date: row.try_get("shoot_date")?,
            expires_at: row.try_get("expires_at")?,
            included_photos: Gallery::included_photos(row)?,
            delivered: row.try_get("delivered")?,
            hero_id: row.try_get("hero_id")?,
        })
    }
}

/// Minimal metadata of a shared gallery safe to show on the client access-code gate before unlock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SharedGalleryGate {
    pub title: Option<String>,
    pub shoot_date: Option<Date>,
    pub has_cover: bool,
}

impl From<SharedGallery> for SharedGalleryGate {
    fn from(gallery: SharedGallery) -> Self {
        Self {
            title: gallery.title,
            shoot_date: gallery.shoot_date,
            has_cover: gallery.hero_id.is_some(),
        }
    }
}

/// A gallery reached through its private link: where its photos live, and what the client sees.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SharedAccess {
    #[sqlx(flatten)]
    pub location: PhotoLocation,
    #[sqlx(flatten)]
    pub gallery: SharedGallery,
    /// Whether the client sees watermarked proofs; kept out of [`SharedGallery`] because the
    /// client page never needs it.
    pub watermark_enabled: bool,
    /// Encrypted 6-digit access code when the gallery is code-protected; kept out of
    /// [`SharedGallery`] so client JSON never serializes ciphertext.
    pub access_code_ciphertext: Option<EncryptedAccessCode>,
    /// The linked contact when reached through a personal gallery-contact link, or `None` on
    /// the gallery's general link. Kept out of [`SharedGallery`] so client responses never
    /// serialize it.
    pub contact_id: Option<ContactId>,
}
