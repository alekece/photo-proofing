use std::future::{self, Ready};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web};
use derive_more::{Deref, FromStr};
use pp_core::types::token::Token;
use sha2::{Digest, Sha256};

use crate::{api::ApiState, routes::auth::AuthError};

/// Size of a SHA-256 digest.
const DIGEST_BYTES: usize = 32;

/// The opaque random value carried by the session cookie.
///
/// 256 bits, far beyond what online or offline guessing can reach, and the size of a SHA-256
/// digest so [`TokenHash`] stores it without truncation. Only the hash is ever stored.
#[derive(Debug, Clone, PartialEq, Eq, Deref, FromStr)]
pub(crate) struct SessionToken(Token<DIGEST_BYTES>);

impl SessionToken {
    pub fn generate() -> Self {
        Self(Token::generate())
    }

    pub fn digest(&self) -> TokenHash {
        TokenHash(Sha256::digest(self.as_str().as_bytes()).into())
    }
}

/// The request's session cookie, or [`AuthError::Unauthorized`] when it is absent or
/// malformed. Handlers that accept a missing cookie take `Option<SessionToken>`.
impl FromRequest for SessionToken {
    type Error = AuthError;
    type Future = Ready<Result<Self, AuthError>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        let Some(state) = request.app_data::<web::Data<ApiState>>() else {
            return future::ready(Err(AuthError::Unavailable));
        };

        future::ready(
            request
                .cookie(state.cookie_name())
                .and_then(|cookie| cookie.value().parse().ok())
                .ok_or(AuthError::Unauthorized),
        )
    }
}

/// SHA-256 digest of a [`SessionToken`]: the database key for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenHash([u8; DIGEST_BYTES]);

impl TokenHash {
    pub fn as_bytes(&self) -> &[u8; DIGEST_BYTES] {
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
}
