use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub(super) fn hash_password(password: &str) -> Result<String, ()> {
    let mut salt = [0u8; 16];
    rand::rng().fill_bytes(&mut salt);
    let salt = SaltString::encode_b64(&salt).map_err(|_| ())?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| ())
}

pub(super) fn verify_password(password: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded).is_ok_and(|hash| {
        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .is_ok()
    })
}

pub(super) fn new_token() -> (String, [u8; 32]) {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let hash = Sha256::digest(token.as_bytes()).into();
    (token, hash)
}

pub(super) fn token_hash(token: &str) -> Option<[u8; 32]> {
    if token.len() != 43 {
        return None;
    }
    let decoded = URL_SAFE_NO_PAD.decode(token).ok()?;
    if decoded.len() != 32 || URL_SAFE_NO_PAD.encode(&decoded) != token {
        return None;
    }
    Some(Sha256::digest(token.as_bytes()).into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_canonical_and_hashes_match() {
        let (token, hash) = new_token();
        assert_eq!(token_hash(&token), Some(hash));
        assert_ne!(new_token().0, token);
        assert!(token_hash("bad").is_none());
        assert!(token_hash(&"a".repeat(1000)).is_none());
    }

    #[test]
    fn passwords_are_salted_and_not_normalized() {
        let password = " a long password ";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash));
        assert!(!verify_password(password.trim(), &hash));
        assert_ne!(hash, hash_password(password).unwrap());
    }
}
