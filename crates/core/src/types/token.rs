use std::{fmt, str::FromStr};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The text is not a canonical random token.
#[derive(Debug, Error)]
#[error("malformed token")]
pub struct ParseTokenError;

/// An unguessable value of `BYTES` random bytes, written as unpadded URL-safe base64.
///
/// It is a bearer credential, so [`Debug`](fmt::Debug) never prints it. Parsing accepts only
/// the canonical encoding, so malformed values are rejected before touching the database.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
#[serde(try_from = "String")]
pub struct Token<const BYTES: usize>(String);

impl<const BYTES: usize> Token<BYTES> {
    /// Length of the encoded form: 6 bits per character, rounded up.
    const ENCODED_LEN: usize = (BYTES * 8).div_ceil(6);

    pub fn generate() -> Self {
        let mut bytes = [0u8; BYTES];

        rand::rng().fill_bytes(&mut bytes);

        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<const BYTES: usize> fmt::Debug for Token<BYTES> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Token(..)")
    }
}

impl<const BYTES: usize> FromStr for Token<BYTES> {
    type Err = ParseTokenError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != Self::ENCODED_LEN {
            return Err(ParseTokenError);
        }

        let decoded = URL_SAFE_NO_PAD.decode(value).map_err(|_| ParseTokenError)?;

        // Re-encoding rejects non-canonical trailing bits that decode to the same bytes.
        if decoded.len() != BYTES || URL_SAFE_NO_PAD.encode(&decoded) != value {
            return Err(ParseTokenError);
        }

        Ok(Self(value.to_owned()))
    }
}

impl<const BYTES: usize> TryFrom<String> for Token<BYTES> {
    type Error = ParseTokenError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Token16 = Token<16>;

    #[test]
    fn parsing_accepts_only_canonical_tokens() {
        let token = Token16::generate();

        assert_eq!(token.as_str().len(), 22);
        assert_eq!(token.as_str().parse::<Token16>().unwrap(), token);

        // The last character of a 16-byte token only carries 2 of its 6 bits, so 'B' (000001)
        // decodes to the same bytes as 'A' but is not canonical.
        let non_canonical = format!("{}B", "A".repeat(21));

        for invalid in [
            "",
            "short",
            &"A".repeat(23),
            &format!("{}+", "A".repeat(21)),
            &format!("{}/", "A".repeat(21)),
            &format!("{}=", "A".repeat(21)),
            non_canonical.as_str(),
        ] {
            assert!(invalid.parse::<Token16>().is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn debug_does_not_print_the_token() {
        let token = Token16::generate();

        assert!(!format!("{token:?}").contains(token.as_str()));
    }
}
