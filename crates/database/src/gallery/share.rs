//! The private link token of a gallery and the client's view of a shared gallery.

use derive_more::{Deref, FromStr};
use pp_core::{
    notifications::PhotoId,
    types::{timestamp::Timestamp, token::Token},
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};

use super::GalleryLayout;
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct SharedGallery {
    /// `None` means the gallery has no title yet.
    pub title: Option<String>,
    pub layout: GalleryLayout,
    /// `None` means the gallery never expires.
    pub expires_at: Option<Timestamp>,
    /// The gallery's hero photo, only while it is ready.
    pub hero_photo_id: Option<PhotoId>,
}

/// A gallery reached through its private link: where its photos live, and what the client sees.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct SharedAccess {
    #[sqlx(flatten)]
    pub location: PhotoLocation,
    #[sqlx(flatten)]
    pub gallery: SharedGallery,
}
