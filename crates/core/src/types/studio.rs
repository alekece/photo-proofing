//! Studio, cover, and invitation value types shared across database, API, and email crates.

use serde::{Deserialize, Serialize};
use thiserror::Error;

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

/// A focal-point percentage outside `0..=100`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("focal coordinate {0} is out of range")]
pub struct FocalCoordinateError(pub u8);

/// Percentage coordinate along one cover axis, bounded to `0..=100`.
///
/// ```
/// use pp_core::types::studio::FocalCoordinate;
///
/// assert_eq!(FocalCoordinate::try_from(0).unwrap().get(), 0);
/// assert_eq!(FocalCoordinate::try_from(100).unwrap().get(), 100);
/// assert!(FocalCoordinate::try_from(101).is_err());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct FocalCoordinate(i16);

impl FocalCoordinate {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(100);
    pub const DEFAULT_X: Self = Self(50);
    pub const DEFAULT_Y: Self = Self(35);

    /// The percentage coordinate, `0` to `100`.
    pub const fn get(self) -> u8 {
        self.0 as u8
    }
}

impl TryFrom<u8> for FocalCoordinate {
    type Error = FocalCoordinateError;

    fn try_from(coordinate: u8) -> Result<Self, Self::Error> {
        if coordinate > Self::MAX.get() {
            return Err(FocalCoordinateError(coordinate));
        }

        Ok(Self(i16::from(coordinate)))
    }
}

impl From<FocalCoordinate> for u8 {
    fn from(coordinate: FocalCoordinate) -> Self {
        coordinate.get()
    }
}

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
            x: FocalCoordinate::DEFAULT_X,
            y: FocalCoordinate::DEFAULT_Y,
        }
    }
}
