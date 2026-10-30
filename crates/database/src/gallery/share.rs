//! The private link token of a gallery and the client's view of a shared gallery.

use std::{fmt, str::FromStr};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use derive_more::{Display, Error};
use rand::RngCore;
use serde::Serialize;
use sqlx::{FromRow, Type};

use super::GalleryLayout;
use crate::types::Timestamp;

/// Random bytes in a share token: 128 bits is out of reach of online guessing, and keeps the
/// link short enough to paste anywhere.
const TOKEN_BYTES: usize = 16;

/// Length of [`TOKEN_BYTES`] bytes in unpadded base64: 6 bits per character, rounded up.
const TOKEN_ENCODED_LEN: usize = (TOKEN_BYTES * 8).div_ceil(6);

/// The text is not a canonical share token.
#[derive(Debug, Display, Error)]
#[display("malformed share token")]
pub struct ParseShareTokenError;

/// The unguessable value that makes up a gallery's private link.
///
/// It is stored as is, not hashed, because sharing again must show the same link. It is a
/// bearer credential, so [`Debug`](fmt::Debug) never prints it. Parsing accepts only the
/// canonical encoding, so malformed link segments are rejected before touching the database.
#[derive(Clone, PartialEq, Eq, Serialize, Type)]
#[serde(transparent)]
#[sqlx(transparent)]
pub struct ShareToken(String);

impl ShareToken {
    pub fn generate() -> Self {
        let mut bytes = [0u8; TOKEN_BYTES];

        rand::rng().fill_bytes(&mut bytes);

        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ShareToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ShareToken(..)")
    }
}

impl FromStr for ShareToken {
    type Err = ParseShareTokenError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != TOKEN_ENCODED_LEN {
            return Err(ParseShareTokenError);
        }

        let decoded = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| ParseShareTokenError)?;

        // Re-encoding rejects non-canonical trailing bits that decode to the same bytes.
        if decoded.len() != TOKEN_BYTES || URL_SAFE_NO_PAD.encode(&decoded) != value {
            return Err(ParseShareTokenError);
        }

        Ok(Self(value.to_owned()))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_accepts_only_canonical_tokens() {
        let token = ShareToken::generate();

        assert_eq!(token.as_str().len(), 22);
        assert_eq!(token.as_str().parse::<ShareToken>().unwrap(), token);

        // The last character of a 16-byte token only carries 2 of its 6 bits, so 'B' (000001)
        // decodes to the same bytes as 'A' but is not canonical.
        let non_canonical = format!("{}B", &"A".repeat(21));

        for invalid in [
            "",
            "short",
            &"A".repeat(23),
            &format!("{}+", "A".repeat(21)),
            &format!("{}/", "A".repeat(21)),
            &format!("{}=", "A".repeat(21)),
            non_canonical.as_str(),
        ] {
            assert!(invalid.parse::<ShareToken>().is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn debug_does_not_print_the_token() {
        let token = ShareToken::generate();

        assert!(!format!("{token:?}").contains(token.as_str()));
    }
}
