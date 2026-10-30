use std::{
    future::{self, Ready},
    str::FromStr,
};

use actix_web::{FromRequest, HttpRequest, dev::Payload};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use derive_more::{Display, Error};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::routes::auth::AuthError;

const TOKEN_BYTES: usize = 32;
const TOKEN_ENCODED_LEN: usize = 43;

/// The cookie value is not a canonical 32-byte token.
#[derive(Debug, Display, Error)]
#[display("malformed session token")]
pub struct ParseTokenError;

/// The opaque random value carried by the session cookie.
///
/// Only its [`TokenHash`] is ever stored. Parsing accepts the canonical URL-safe base64 of
/// exactly 32 bytes, so malformed cookies are rejected before touching the database.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SessionToken(String);

/// SHA-256 digest of a [`SessionToken`]: the database key for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenHash([u8; 32]);

impl SessionToken {
    pub const COOKIE_NAME: &str = "pp_session";

    pub fn generate() -> Self {
        let mut bytes = [0u8; TOKEN_BYTES];
        rand::rng().fill_bytes(&mut bytes);

        Self(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn digest(&self) -> TokenHash {
        TokenHash(Sha256::digest(self.0.as_bytes()).into())
    }
}

impl FromStr for SessionToken {
    type Err = ParseTokenError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != TOKEN_ENCODED_LEN {
            return Err(ParseTokenError);
        }

        let decoded = URL_SAFE_NO_PAD.decode(value).map_err(|_| ParseTokenError)?;

        // Re-encoding rejects non-canonical trailing bits that decode to the same bytes.
        if decoded.len() != TOKEN_BYTES || URL_SAFE_NO_PAD.encode(&decoded) != value {
            return Err(ParseTokenError);
        }

        Ok(Self(value.to_owned()))
    }
}

/// The request's session cookie, or [`AuthError::Unauthorized`] when it is absent or
/// malformed. Handlers that accept a missing cookie take `Option<SessionToken>`.
impl FromRequest for SessionToken {
    type Error = AuthError;
    type Future = Ready<Result<Self, AuthError>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        future::ready(
            request
                .cookie(Self::COOKIE_NAME)
                .and_then(|cookie| cookie.value().parse().ok())
                .ok_or(AuthError::Unauthorized),
        )
    }
}

impl TokenHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_round_trip_through_their_canonical_form() {
        let token = SessionToken::generate();
        let parsed: SessionToken = token.as_str().parse().unwrap();

        assert_eq!(parsed.digest(), token.digest());
        assert_ne!(SessionToken::generate().as_str(), token.as_str());
    }

    #[test]
    fn malformed_tokens_are_rejected() {
        assert!("bad".parse::<SessionToken>().is_err());
        assert!("a".repeat(1000).parse::<SessionToken>().is_err());
        assert!("A".repeat(43).parse::<SessionToken>().is_ok());
        // The final character carries non-zero padding bits, so it is not canonical.
        assert!(
            format!("{}B", "A".repeat(42))
                .parse::<SessionToken>()
                .is_err()
        );
    }
}
