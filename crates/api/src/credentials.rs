//! Secrets handled by the authentication endpoints: session tokens and password hashes.
//!
//! Each secret has one type that owns its encoding, so call sites cannot hash a raw string
//! that was never validated or store a token instead of its digest.

use std::str::FromStr;

use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHash as PhcHash, SaltString},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use derive_more::{Display, Error};
use rand::RngCore;
use sha2::{Digest, Sha256};

const TOKEN_BYTES: usize = 32;
const TOKEN_ENCODED_LEN: usize = 43;

/// The opaque random value carried by the session cookie.
///
/// Only its [`TokenHash`] is ever stored. Parsing accepts the canonical URL-safe base64 of
/// exactly 32 bytes, so malformed cookies are rejected before touching the database.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

/// SHA-256 digest of a [`SessionToken`]: the database key for a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenHash([u8; 32]);

/// The cookie value is not a canonical 32-byte token.
#[derive(Debug, Display, Error)]
#[display("malformed session token")]
pub struct ParseTokenError;

impl SessionToken {
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

impl TokenHash {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
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

/// Argon2id hash with a fresh random salt, stored as a PHC string.
///
/// Passwords are hashed exactly as typed: no trimming or normalization. Hashing is CPU and
/// memory heavy; callers run it off the async executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

/// The password could not be hashed.
#[derive(Debug, Display, Error)]
#[display("password hashing failed")]
pub struct HashError;

impl PasswordHash {
    pub fn new(password: &str) -> Result<Self, HashError> {
        let mut salt = [0u8; 16];
        rand::rng().fill_bytes(&mut salt);

        let salt = SaltString::encode_b64(&salt).map_err(|_| HashError)?;

        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| Self(hash.to_string()))
            .map_err(|_| HashError)
    }

    /// Wraps a PHC string read from storage. It is not parsed here: a malformed value simply
    /// fails [`verify`](Self::verify), which reads as a failed login.
    pub fn from_stored(encoded: String) -> Self {
        Self(encoded)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn verify(&self, password: &str) -> bool {
        PhcHash::new(&self.0).is_ok_and(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .is_ok()
        })
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

    #[test]
    fn passwords_are_salted_and_not_normalized() {
        let password = " a long password ";
        let hash = PasswordHash::new(password).unwrap();

        assert!(hash.verify(password));
        assert!(!hash.verify(password.trim()));
        assert_ne!(hash, PasswordHash::new(password).unwrap());
    }

    #[test]
    fn a_malformed_stored_hash_fails_verification() {
        assert!(!PasswordHash::from_stored("not a hash".into()).verify("anything"));
    }
}
