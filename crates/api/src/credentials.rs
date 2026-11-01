//! Secrets handled by the authentication endpoints: passwords and their hashes.
//!
//! Each secret has one type that owns its encoding, so call sites cannot hash a raw string
//! that was never validated. None of them implements `Debug` for the raw value, so they
//! cannot leak through logging. Session tokens live with the request extractors.

use argon2::{
    Argon2, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHash as PhcHash, SaltString},
};
use rand::Rng;
use serde::Deserialize;
use thiserror::Error;

use crate::extractors::session_token::SessionToken;

/// The password length is outside the accepted range.
#[derive(Debug, Error)]
#[error("password must be 12–128 characters")]
pub struct ParsePasswordError;

/// The password could not be hashed.
#[derive(Debug, Error)]
#[error("password hashing failed")]
pub struct HashError;

/// A password of 12–128 characters, kept exactly as typed: no trimming or normalization.
#[derive(Deserialize)]
#[serde(try_from = "String")]
pub struct Password(String);

impl Password {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Password {
    type Error = ParsePasswordError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !(12..=128).contains(&value.chars().count()) {
            return Err(ParsePasswordError);
        }

        Ok(Self(value))
    }
}

/// Argon2id hash with a fresh random salt, stored as a PHC string.
///
/// Hashing is CPU and memory heavy; callers run it off the async executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn new(password: &str) -> Result<Self, HashError> {
        let mut salt = [0u8; 16];

        rand::rng().fill_bytes(&mut salt);

        // `SaltString` is the PHC-format (base64) salt the `argon2` crate requires; 16 random
        // bytes is the recommended salt length.
        let salt = SaltString::encode_b64(&salt).map_err(|_| HashError)?;

        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| Self(hash.to_string()))
            .map_err(|_| HashError)
    }

    /// Hash of a random password nobody knows, for verifications that must cost as much as a
    /// real one but can never succeed.
    pub fn generate() -> Result<Self, HashError> {
        Self::new(SessionToken::generate().as_str())
    }

    /// Wraps a PHC string read from storage. It is not parsed here: a malformed value simply
    /// fails [`verify`](Self::verify), which reads as a failed login.
    pub fn unchecked_new(encoded: String) -> Self {
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
    fn passwords_must_be_12_to_128_characters() {
        assert!(Password::try_from("x".repeat(11)).is_err());
        assert!(Password::try_from("x".repeat(12)).is_ok());
        assert!(Password::try_from("x".repeat(128)).is_ok());
        assert!(Password::try_from("x".repeat(129)).is_err());
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
        assert!(!PasswordHash::unchecked_new("not a hash".into()).verify("anything"));
    }
}
