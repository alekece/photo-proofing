//! The private link token of a gallery and the client's view of a shared gallery.

use std::num::NonZeroUsize;

use derive_more::{Deref, FromStr};
use pp_core::{
    notifications::PhotoId,
    types::{timestamp::Timestamp, token::Token},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Row, Type, postgres::PgRow};

use super::{GalleryLayout, included_photos};
use crate::photo::PhotoLocation;

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
            expires_at: row.try_get("expires_at")?,
            included_photos: included_photos(row)?,
            delivered: row.try_get("delivered")?,
            hero_id: row.try_get("hero_id")?,
        })
    }
}

/// A gallery reached through its private link: where its photos live, and what the client sees.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SharedAccess {
    #[sqlx(flatten)]
    pub location: PhotoLocation,
    #[sqlx(flatten)]
    pub gallery: SharedGallery,
}
