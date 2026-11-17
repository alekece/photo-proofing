//! The private link token of a gallery and the client's view of a shared gallery.

use std::{future::Future, num::NonZeroUsize, ops::AsyncFnMut};

use derive_more::{Deref, FromStr};
use pp_core::types::token::Token;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Postgres, Row, Transaction, Type, postgres::PgRow};
use time::Date;

use super::{
    CoverLayout, FocalPoint, Gallery, GalleryAccessCode, GalleryLayout, GridDensity, GridSpacing,
    HeadingFont,
};
use crate::{
    contact::ContactId,
    error::{DatabaseError, UniqueViolation},
    photo::{PhotoId, PhotoLocation},
};

/// Outcome of one [`ShareToken`] persistence attempt.
pub(crate) enum TokenAttempt<T> {
    /// Persistence completed (or determined the target entity does not exist).
    Done(T),
    /// Candidate token collided with an existing token.
    Collision,
}

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

    /// Retries `attempt` with fresh tokens from `next_token` whenever a candidate collides with
    /// `violation` or reports [`TokenAttempt::Collision`].
    pub(crate) async fn retry_on_collision<T, F, Fut>(
        violation: UniqueViolation,
        mut next_token: impl FnMut() -> Self,
        mut attempt: F,
    ) -> Result<T, DatabaseError>
    where
        F: FnMut(Self) -> Fut,
        Fut: Future<Output = Result<TokenAttempt<T>, DatabaseError>>,
    {
        loop {
            match attempt(next_token()).await {
                Ok(TokenAttempt::Done(output)) => return Ok(output),
                Ok(TokenAttempt::Collision) => {}
                Err(error) if error.is_unique_violation(violation) => {}
                Err(error) => return Err(error),
            }
        }
    }

    /// Retries `attempt` inside `tx` behind a savepoint so a unique-violation collision rolls back
    /// only the failed candidate without aborting the surrounding transaction.
    pub(crate) async fn retry_in<T>(
        tx: &mut Transaction<'_, Postgres>,
        violation: UniqueViolation,
        mut next_token: impl FnMut() -> Self,
        mut attempt: impl AsyncFnMut(
            &mut Transaction<'_, Postgres>,
            Self,
        ) -> Result<TokenAttempt<T>, DatabaseError>,
    ) -> Result<T, DatabaseError> {
        loop {
            sqlx::query("SAVEPOINT share_token_attempt")
                .execute(&mut **tx)
                .await?;

            match attempt(tx, next_token()).await {
                Ok(TokenAttempt::Done(output)) => {
                    sqlx::query("RELEASE SAVEPOINT share_token_attempt")
                        .execute(&mut **tx)
                        .await?;

                    return Ok(output);
                }
                Ok(TokenAttempt::Collision) => {
                    sqlx::query("ROLLBACK TO SAVEPOINT share_token_attempt")
                        .execute(&mut **tx)
                        .await?;
                }
                Err(error) if error.is_unique_violation(violation) => {
                    sqlx::query("ROLLBACK TO SAVEPOINT share_token_attempt")
                        .execute(&mut **tx)
                        .await?;
                }
                Err(error) => return Err(error),
            }
        }
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
    /// `None` means the gallery never expires; when set, the gallery remains accessible through
    /// the end of that UTC calendar day.
    pub expires_at: Option<Date>,
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
    /// Optional 6-digit access code when the gallery is code-protected; kept out of
    /// [`SharedGallery`] so client JSON never serializes the digits.
    pub access_code: Option<GalleryAccessCode>,
    /// The linked contact when reached through a personal gallery-contact link, or `None` on
    /// the gallery's general link. Kept out of [`SharedGallery`] so client responses never
    /// serialize it.
    pub contact_id: Option<ContactId>,
}
