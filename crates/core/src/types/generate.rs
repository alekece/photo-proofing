//! Identifier generation without exposing the underlying representation to callers.

/// Creates a fresh identifier using the implementing type's generation policy.
///
/// ```
/// use pp_core::{notifications::PhotoId, types::generate::Generate};
///
/// let photo_id = PhotoId::generate();
/// let another_photo_id = PhotoId::generate();
/// assert_ne!(photo_id, another_photo_id);
/// ```
pub trait Generate {
    #[must_use]
    fn generate() -> Self;
}
