//! Value types shared by several tables.

use serde::{Deserialize, Deserializer, Serialize, de::Error as _};
use sqlx::Type;
use time::{OffsetDateTime, UtcOffset};

/// A point in time that is read and written as an RFC 3339 string.
///
/// Parsing normalizes to UTC and rejects instants whose UTC year is outside 0000-9999: such a
/// value would panic when sqlx converts it for Postgres and could not be written back as
/// RFC 3339.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct Timestamp(#[serde(with = "time::serde::rfc3339")] pub OffsetDateTime);

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        time::serde::rfc3339::deserialize(deserializer)?
            .checked_to_offset(UtcOffset::UTC)
            .filter(|instant| instant.year() >= 0)
            .map(Self)
            .ok_or_else(|| D::Error::custom("timestamp out of range"))
    }
}
