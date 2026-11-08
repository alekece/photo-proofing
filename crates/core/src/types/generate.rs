//! Identifier generation without exposing the underlying representation to callers.

/// Creates a fresh identifier using the implementing type's generation policy.
///
/// ```
/// use std::sync::atomic::{AtomicU64, Ordering};
///
/// use pp_core::types::generate::Generate;
///
/// #[derive(Debug, PartialEq)]
/// struct PhotoId(u64);
///
/// impl Generate for PhotoId {
///     fn generate() -> Self {
///         static NEXT: AtomicU64 = AtomicU64::new(0);
///
///         Self(NEXT.fetch_add(1, Ordering::Relaxed))
///     }
/// }
///
/// assert_ne!(PhotoId::generate(), PhotoId::generate());
/// ```
pub trait Generate {
    #[must_use]
    fn generate() -> Self;
}
