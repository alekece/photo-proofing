//! Studio, cover, and invitation value types shared across database, API, and email crates.

use serde::{Deserialize, Serialize};

use super::bounded::BoundedU8;

/// Default horizontal cover crop coordinate (`50%`).
pub const DEFAULT_FOCAL_X: FocalCoordinate = FocalCoordinate::constant::<50>();

/// Default vertical cover crop coordinate (`35%`), keeping portrait faces in frame.
pub const DEFAULT_FOCAL_Y: FocalCoordinate = FocalCoordinate::constant::<35>();

/// How the studio signs the client cover and outgoing invitation cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "sqlx",
    derive(sqlx::Type),
    sqlx(type_name = "cover_signature", rename_all = "snake_case")
)]
pub enum CoverSignature {
    Name,
    Logo,
    Both,
    None,
}

impl CoverSignature {
    pub const fn shows_logo(self) -> bool {
        matches!(self, Self::Logo | Self::Both)
    }

    pub const fn shows_name(self) -> bool {
        matches!(self, Self::Name | Self::Both)
    }
}

/// The language of the photographer app, client pages, and outgoing emails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    feature = "sqlx",
    derive(sqlx::Type),
    sqlx(type_name = "ui_language", rename_all = "snake_case")
)]
pub enum Language {
    En,
    Fr,
}

/// Which gallery lifecycle stage an invitation email announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvitationStage {
    Proofing,
    Finals,
}

/// Percentage coordinate along one cover axis, bounded to `0..=100`.
///
/// ```
/// use pp_core::types::studio::FocalCoordinate;
///
/// assert_eq!(FocalCoordinate::try_from(0).unwrap().get(), 0);
/// assert_eq!(FocalCoordinate::try_from(100).unwrap().get(), 100);
/// assert!(FocalCoordinate::try_from(101).is_err());
/// ```
pub type FocalCoordinate = BoundedU8<0, 100>;

/// Horizontal and vertical cover crop anchor, in `0..=100` percentages from the top-left corner.
///
/// Defaults to `(50, 35)` so portraits keep faces in frame.
///
/// ```
/// use pp_core::types::studio::FocalPoint;
///
/// let point = FocalPoint::default();
/// assert_eq!(point.x.get(), 50);
/// assert_eq!(point.y.get(), 35);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
pub struct FocalPoint {
    #[cfg_attr(feature = "sqlx", sqlx(rename = "hero_focal_x"))]
    pub x: FocalCoordinate,
    #[cfg_attr(feature = "sqlx", sqlx(rename = "hero_focal_y"))]
    pub y: FocalCoordinate,
}

impl Default for FocalPoint {
    fn default() -> Self {
        Self {
            x: DEFAULT_FOCAL_X,
            y: DEFAULT_FOCAL_Y,
        }
    }
}
