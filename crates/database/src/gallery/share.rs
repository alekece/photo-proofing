//! The private link token of a gallery and the client's view of a shared gallery.

use serde::Serialize;
use sqlx::FromRow;

use super::GalleryLayout;
use crate::types::{RandomToken, Timestamp};

/// The unguessable value that makes up a gallery's private link.
///
/// 128 bits is out of reach of online guessing and keeps the link short enough to paste
/// anywhere. It is stored as is, not hashed, because sharing again must show the same link.
pub type ShareToken = RandomToken<16>;

/// What a client holding the link may see of a gallery, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, FromRow)]
pub struct SharedGallery {
    /// `None` means the gallery has no title yet.
    pub title: Option<String>,
    pub layout: GalleryLayout,
    /// `None` means the gallery never expires.
    pub expires_at: Option<Timestamp>,
}
