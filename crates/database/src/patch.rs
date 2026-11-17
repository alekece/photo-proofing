//! Shared serde helper for partial-update (`PATCH`) payloads.

use serde::{Deserialize, Deserializer, de::Error as _};

/// Deserializes a supplied JSON field as `Some(T)` while rejecting explicit `null`.
///
/// Plain `Option<T>` fields treat `null` like a missing field, so `{"layout": null}` or
/// `{"name": null}` would silently leave a non-nullable setting unchanged; fields that cannot
/// be cleared fail deserialization instead.
pub(crate) struct Present;

impl Present {
    pub(crate) fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        // Going through `Option` makes `null` a data error for every `T`; deserializing `T`
        // directly makes some types (such as enums) report it as a syntax error instead.
        Option::<T>::deserialize(deserializer)?
            .map(Some)
            .ok_or_else(|| D::Error::custom("null is not allowed"))
    }
}
