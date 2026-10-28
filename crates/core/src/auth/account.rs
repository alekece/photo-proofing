use std::time::SystemTime;

use crate::auth::password::PasswordHash;

/// The single owner of the gallery. Persistence is added with the database layer.
#[derive(Debug, Clone)]
pub struct PhotographerAccount {
    pub username: String,
    pub password_hash: PasswordHash,
    pub created_at: SystemTime,
}

impl PhotographerAccount {
    pub fn new(
        username: impl Into<String>,
        password_hash: PasswordHash,
        created_at: SystemTime,
    ) -> Self {
        Self {
            username: username.into(),
            password_hash,
            created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::password::{HashingParams, Password, PasswordHasher};

    #[test]
    fn account_debug_never_leaks_the_password_or_its_hash() {
        let hasher = PasswordHasher::new(HashingParams::insecure_for_tests())
            .expect("test params are valid");
        let hash = hasher
            .hash(&Password::new("correct horse battery"))
            .expect("hashing succeeds");
        let account = PhotographerAccount::new("owner", hash, SystemTime::UNIX_EPOCH);
        let output = format!("{account:?}");

        assert!(!output.contains("argon2"));
        assert!(!output.contains("correct horse"));
    }

    #[test]
    fn credential_lifecycle_login_succeeds_then_password_change_invalidates_old() {
        let hasher = PasswordHasher::new(HashingParams::insecure_for_tests())
            .expect("test params are valid");
        let old = Password::new("correct horse battery");
        let new = Password::new("another staple battery");
        let mut account = PhotographerAccount::new(
            "owner",
            hasher.hash(&old).expect("hashing succeeds"),
            SystemTime::UNIX_EPOCH,
        );

        assert!(
            hasher
                .verify(&old, &account.password_hash)
                .expect("verify succeeds")
        );

        account.password_hash = hasher.hash(&new).expect("hashing succeeds");

        assert!(
            !hasher
                .verify(&old, &account.password_hash)
                .expect("verify succeeds")
        );
        assert!(
            hasher
                .verify(&new, &account.password_hash)
                .expect("verify succeeds")
        );
    }
}
