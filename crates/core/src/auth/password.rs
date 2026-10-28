use std::fmt;

use argon2::{
    Algorithm, Argon2, Params, Version,
    password_hash::{PasswordHasher as _, PasswordVerifier as _, phc},
};
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

pub const MIN_PASSWORD_LEN: usize = 12;
/// Bounds the work an unauthenticated caller can force on the server.
pub const MAX_PASSWORD_LEN: usize = 1024;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password must be at least {MIN_PASSWORD_LEN} bytes long")]
    TooShort,
    #[error("password must be at most {MAX_PASSWORD_LEN} bytes long")]
    TooLong,
    #[error("invalid argon2 parameters")]
    InvalidParams,
    #[error("password hashing failed")]
    Hashing,
    #[error("stored password hash is malformed")]
    MalformedHash,
}

/// Argon2id cost parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashingParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for HashingParams {
    /// OWASP-recommended minimum for Argon2id (19 MiB, 2 iterations, 1 lane).
    fn default() -> Self {
        Self {
            memory_kib: 19 * 1024,
            iterations: 2,
            parallelism: 1,
        }
    }
}

impl HashingParams {
    /// Minimal cost for fast tests; never use in production.
    pub const fn insecure_for_tests() -> Self {
        Self {
            memory_kib: 8,
            iterations: 1,
            parallelism: 1,
        }
    }
}

/// A plain-text password; redacted in `Debug` and zeroized on drop.
#[derive(Clone)]
pub struct Password(SecretString);

impl Password {
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::from(value.into()))
    }

    fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    fn check_length(&self) -> Result<(), PasswordError> {
        match self.expose().len() {
            len if len < MIN_PASSWORD_LEN => Err(PasswordError::TooShort),
            len if len > MAX_PASSWORD_LEN => Err(PasswordError::TooLong),
            _ => Ok(()),
        }
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password([REDACTED])")
    }
}

/// A PHC-formatted Argon2id hash (`$argon2id$...`), the only password
/// representation that may be persisted. Redacted in `Debug`.
#[derive(Clone)]
pub struct PasswordHash(SecretString);

impl PasswordHash {
    /// Wraps a PHC string loaded from storage, rejecting malformed input.
    pub fn from_phc(value: impl Into<String>) -> Result<Self, PasswordError> {
        let value = value.into();
        phc::PasswordHash::new(&value).map_err(|_| PasswordError::MalformedHash)?;

        Ok(Self(SecretString::from(value)))
    }

    pub fn as_phc(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for PasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PasswordHash([REDACTED])")
    }
}

pub struct PasswordHasher {
    argon2: Argon2<'static>,
    // Verified against when the account is unknown so that both paths cost the same.
    decoy: PasswordHash,
}

impl PasswordHasher {
    pub fn new(params: HashingParams) -> Result<Self, PasswordError> {
        let params = Params::new(
            params.memory_kib,
            params.iterations,
            params.parallelism,
            None,
        )
        .map_err(|_| PasswordError::InvalidParams)?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let decoy = hash_with(&argon2, "decoy-password-never-matches")?;

        Ok(Self { argon2, decoy })
    }

    /// Hashes with a fresh random salt.
    pub fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordError> {
        password.check_length()?;

        hash_with(&self.argon2, password.expose())
    }

    /// Returns `Ok(false)` on mismatch; errors only for an unusable stored hash.
    /// Comparison is constant-time and uses the parameters embedded in `hash`.
    pub fn verify(&self, password: &Password, hash: &PasswordHash) -> Result<bool, PasswordError> {
        let parsed =
            phc::PasswordHash::new(hash.as_phc()).map_err(|_| PasswordError::MalformedHash)?;

        // Out-of-range lengths can never match a hash created through `hash`.
        if password.check_length().is_err() {
            return Ok(false);
        }

        Ok(self
            .argon2
            .verify_password(password.expose().as_bytes(), &parsed)
            .is_ok())
    }

    /// Spends the cost of a verification for a login attempt without a known account.
    pub fn verify_decoy(&self, password: &Password) {
        let _ = self.verify(password, &self.decoy);
    }
}

impl fmt::Debug for PasswordHasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordHasher").finish_non_exhaustive()
    }
}

fn hash_with(argon2: &Argon2<'_>, password: &str) -> Result<PasswordHash, PasswordError> {
    let hash = argon2
        .hash_password(password.as_bytes())
        .map_err(|_| PasswordError::Hashing)?;

    Ok(PasswordHash(SecretString::from(hash.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "correct horse battery";

    fn hasher() -> PasswordHasher {
        PasswordHasher::new(HashingParams::insecure_for_tests()).expect("test params are valid")
    }

    #[test]
    fn hash_is_argon2id_phc_and_not_the_plain_password() {
        let hash = hasher()
            .hash(&Password::new(PASSWORD))
            .expect("hashing succeeds");

        assert!(hash.as_phc().starts_with("$argon2id$v=19$"));
        assert!(!hash.as_phc().contains("correct horse"));
    }

    #[test]
    fn hashing_twice_uses_distinct_salts() {
        let hasher = hasher();
        let password = Password::new(PASSWORD);

        let first = hasher.hash(&password).expect("hashing succeeds");
        let second = hasher.hash(&password).expect("hashing succeeds");

        assert_ne!(first.as_phc(), second.as_phc());
    }

    #[test]
    fn verify_accepts_right_and_rejects_wrong_password() {
        let hasher = hasher();
        let hash = hasher
            .hash(&Password::new(PASSWORD))
            .expect("hashing succeeds");

        assert!(
            hasher
                .verify(&Password::new(PASSWORD), &hash)
                .expect("verify succeeds")
        );
        assert!(
            !hasher
                .verify(&Password::new("wrong horse battery!"), &hash)
                .expect("verify succeeds")
        );
    }

    #[test]
    fn verify_uses_params_embedded_in_the_hash() {
        let hash = hasher()
            .hash(&Password::new(PASSWORD))
            .expect("hashing succeeds");
        let other = PasswordHasher::new(HashingParams {
            memory_kib: 16,
            iterations: 2,
            parallelism: 1,
        })
        .expect("params are valid");

        assert!(
            other
                .verify(&Password::new(PASSWORD), &hash)
                .expect("verify succeeds")
        );
    }

    #[test]
    fn password_length_is_bounded() {
        let hasher = hasher();

        assert!(matches!(
            hasher.hash(&Password::new("short")),
            Err(PasswordError::TooShort)
        ));
        assert!(matches!(
            hasher.hash(&Password::new("a".repeat(MAX_PASSWORD_LEN + 1))),
            Err(PasswordError::TooLong)
        ));
    }

    #[test]
    fn verify_rejects_out_of_range_password_without_hashing_it() {
        let hasher = hasher();
        let hash = hasher
            .hash(&Password::new(PASSWORD))
            .expect("hashing succeeds");

        assert!(
            !hasher
                .verify(&Password::new("a".repeat(MAX_PASSWORD_LEN + 1)), &hash)
                .expect("verify succeeds")
        );
    }

    #[test]
    fn malformed_stored_hash_is_rejected() {
        assert!(matches!(
            PasswordHash::from_phc("not-a-hash"),
            Err(PasswordError::MalformedHash)
        ));
    }

    #[test]
    fn stored_hash_round_trips_through_phc() {
        let hasher = hasher();
        let hash = hasher
            .hash(&Password::new(PASSWORD))
            .expect("hashing succeeds");
        let restored = PasswordHash::from_phc(hash.as_phc()).expect("valid phc");

        assert!(
            hasher
                .verify(&Password::new(PASSWORD), &restored)
                .expect("verify succeeds")
        );
    }

    #[test]
    fn invalid_params_are_rejected() {
        let params = HashingParams {
            memory_kib: 0,
            iterations: 0,
            parallelism: 0,
        };

        assert!(matches!(
            PasswordHasher::new(params),
            Err(PasswordError::InvalidParams)
        ));
    }

    #[test]
    fn secrets_never_appear_in_debug_output() {
        let hasher = hasher();
        let password = Password::new(PASSWORD);
        let hash = hasher.hash(&password).expect("hashing succeeds");
        let output = format!("{password:?} {hash:?} {hasher:?}");

        assert!(!output.contains("correct horse"));
        assert!(!output.contains("argon2id"));
        assert!(output.contains("REDACTED"));
    }
}
