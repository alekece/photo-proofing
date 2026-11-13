//! Per-gallery 6-digit access-code encryption at rest and stateless client unlock cookies.
//!
//! [`AccessCodeCipher`] derives two domain-separated 256-bit subkeys via HMAC-SHA256 from the
//! operator's [`EncryptionKey`]:
//! - a `ChaCha20-Poly1305` AEAD subkey that encrypts the 6 ASCII digits of a
//!   [`GalleryAccessCode`] with a random 96-bit nonce and `gallery_id` as AAD (`34` bytes);
//! - an `HMAC-SHA256` subkey that signs and verifies the `pp_gallery_unlock` `HttpOnly` cookie,
//!   binding up to [`MAX_UNLOCKED_GALLERIES`] `(gallery_id, sha256(ciphertext))` entries so
//!   unlocking one gallery keeps previously unlocked galleries open and works identically across
//!   the general gallery link and personal contact links.
//!
//! None of the types in this module prints raw key material, digits, ciphertext, or cookie
//! tokens in [`fmt::Debug`].

use std::{ffi::OsStr, fmt, str::FromStr};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit as _, Payload},
};
use clap::{Arg, Command, Error as ClapError, builder::TypedValueParser, error::ErrorKind};
use hmac::{Hmac, Mac};
pub use pp_core::types::access_code::{
    ACCESS_CODE_DIGITS, GalleryAccessCode, ParseAccessCodeError,
};
use pp_database::gallery::{ENCRYPTED_ACCESS_CODE_LEN, EncryptedAccessCode, GalleryId};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

/// Name of the `HttpOnly` cookie recording unlocked galleries in a client browser.
pub const UNLOCK_COOKIE_NAME: &str = "pp_gallery_unlock";

/// Maximum `(gallery_id, ciphertext_digest)` entries retained in [`UNLOCK_COOKIE_NAME`].
pub const MAX_UNLOCKED_GALLERIES: usize = 16;

/// Size in bytes of the operator encryption key.
pub const KEY_BYTES: usize = 32;

const NONCE_BYTES: usize = 12;
const DIGEST_BYTES: usize = 32;
const UUID_BYTES: usize = 16;
const UNLOCK_RECORD_BYTES: usize = UUID_BYTES + DIGEST_BYTES;
const COOKIE_VERSION: u8 = 1;
const MIN_COOKIE_RAW_BYTES: usize = 1 + UNLOCK_RECORD_BYTES + DIGEST_BYTES;
const MAX_COOKIE_RAW_BYTES: usize =
    1 + (MAX_UNLOCKED_GALLERIES * UNLOCK_RECORD_BYTES) + DIGEST_BYTES;
const MAX_COOKIE_ENCODED_LEN: usize = (MAX_COOKIE_RAW_BYTES * 8).div_ceil(6);
const BASE64_KEY_LEN: usize = (KEY_BYTES * 8).div_ceil(6);
const AEAD_SUBKEY_LABEL: &[u8] = b"pp-gallery-access-code-aead-v1";
const COOKIE_SUBKEY_LABEL: &[u8] = b"pp-gallery-access-code-cookie-v1";

type HmacSha256 = Hmac<Sha256>;

/// The configured key is neither 64 hex digits nor canonical 43-char unpadded base64url.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("encryption key must be 32 bytes as 64 hex digits or 43 unpadded base64url characters")]
pub struct ParseEncryptionKeyError;

/// Encryption or decryption of a gallery access code failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("gallery access code cipher failed")]
pub struct AccessCodeCryptoError;

/// Validated 256-bit operator key for `[gallery_password] encryption_key`.
///
/// [`Debug`](fmt::Debug) redacts the key bytes so configuration dumps never leak secrets.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EncryptionKey([u8; KEY_BYTES]);

impl EncryptionKey {
    pub fn from_bytes(bytes: [u8; KEY_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    pub const fn value_parser() -> EncryptionKeyValueParser {
        EncryptionKeyValueParser
    }

    /// Canonical 43-character unpadded base64url form used internally by config merging;
    /// [`fmt::Display`] is intentionally omitted so `%key` cannot log key bytes.
    pub(crate) fn encode_base64url(self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    fn decode_nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    fn decode_hex(value: &str) -> Option<[u8; KEY_BYTES]> {
        let (pairs, remainder) = value.as_bytes().as_chunks::<2>();

        if pairs.len() != KEY_BYTES || !remainder.is_empty() {
            return None;
        }

        let mut bytes = [0u8; KEY_BYTES];

        for (index, &[first, second]) in pairs.iter().enumerate() {
            let high = Self::decode_nibble(first)?;
            let low = Self::decode_nibble(second)?;

            bytes[index] = (high << 4) | low;
        }

        Some(bytes)
    }

    fn decode_base64url(value: &str) -> Option<[u8; KEY_BYTES]> {
        if value.len() != BASE64_KEY_LEN {
            return None;
        }

        let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;

        if bytes.len() != KEY_BYTES || URL_SAFE_NO_PAD.encode(&bytes) != value {
            return None;
        }

        bytes.try_into().ok()
    }
}

impl fmt::Debug for EncryptionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EncryptionKey(..)")
    }
}

impl FromStr for EncryptionKey {
    type Err = ParseEncryptionKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let bytes = Self::decode_hex(value)
            .or_else(|| Self::decode_base64url(value))
            .ok_or(ParseEncryptionKeyError)?;

        Ok(Self(bytes))
    }
}

impl TryFrom<String> for EncryptionKey {
    type Error = ParseEncryptionKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<EncryptionKey> for String {
    fn from(key: EncryptionKey) -> Self {
        key.encode_base64url()
    }
}

/// Clap parser for [`EncryptionKey`] that omits [`clap::error::ContextKind::InvalidValue`] so
/// malformed `PP_GALLERY_PASSWORD_KEY` values are never echoed to startup logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncryptionKeyValueParser;

impl TypedValueParser for EncryptionKeyValueParser {
    type Value = EncryptionKey;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, ClapError> {
        match value
            .to_str()
            .ok_or(ParseEncryptionKeyError)
            .and_then(EncryptionKey::from_str)
        {
            Ok(key) => Ok(key),
            Err(error) => {
                let flag =
                    arg.map_or_else(|| "--gallery-password-key".to_owned(), ToString::to_string);

                Err(cmd.clone().error(
                    ErrorKind::ValueValidation,
                    format!("invalid value for '{flag}': {error}"),
                ))
            }
        }
    }
}

/// Fixed-width `(gallery_id, sha256(ciphertext))` entry inside [`UNLOCK_COOKIE_NAME`].
#[derive(Clone, Copy)]
struct UnlockRecord {
    gallery_id: GalleryId,
    digest: [u8; DIGEST_BYTES],
}

impl UnlockRecord {
    fn from_bytes(record: &[u8; UNLOCK_RECORD_BYTES]) -> Self {
        let mut uuid_bytes = [0u8; UUID_BYTES];
        let mut digest = [0u8; DIGEST_BYTES];

        uuid_bytes.copy_from_slice(&record[..UUID_BYTES]);
        digest.copy_from_slice(&record[UUID_BYTES..]);

        Self {
            gallery_id: GalleryId(Uuid::from_bytes(uuid_bytes)),
            digest,
        }
    }

    fn append_to(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.gallery_id.0.as_bytes());
        buffer.extend_from_slice(&self.digest);
    }
}

/// Domain-separated AEAD cipher and unlock-cookie signer derived from [`EncryptionKey`].
///
/// [`Debug`](fmt::Debug) redacts all derived subkeys.
#[derive(Clone)]
pub struct AccessCodeCipher {
    aead: ChaCha20Poly1305,
    cookie_key: [u8; KEY_BYTES],
}

impl AccessCodeCipher {
    pub fn new(master_key: &EncryptionKey) -> Self {
        let aead_key = Self::derive_subkey(master_key.as_bytes(), AEAD_SUBKEY_LABEL);
        let cookie_key = Self::derive_subkey(master_key.as_bytes(), COOKIE_SUBKEY_LABEL);

        Self {
            aead: ChaCha20Poly1305::new(&aead_key.into()),
            cookie_key,
        }
    }

    /// Encrypts the 6 ASCII digits of `code` under a fresh 96-bit random nonce with
    /// `gallery_id` bound as AEAD associated data (`34` bytes total).
    pub fn encrypt(
        &self,
        gallery_id: GalleryId,
        code: &GalleryAccessCode,
    ) -> Result<EncryptedAccessCode, AccessCodeCryptoError> {
        let mut nonce_bytes = [0u8; NONCE_BYTES];

        rand::rng().fill_bytes(&mut nonce_bytes);

        let nonce = Nonce::from(nonce_bytes);
        let payload = Payload {
            msg: code.as_bytes(),
            aad: gallery_id.0.as_bytes(),
        };
        let ciphertext = self
            .aead
            .encrypt(&nonce, payload)
            .map_err(|_| AccessCodeCryptoError)?;

        let mut combined = Vec::with_capacity(ENCRYPTED_ACCESS_CODE_LEN);

        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);

        EncryptedAccessCode::try_from(combined).map_err(|_| AccessCodeCryptoError)
    }

    /// Decrypts and validates `ciphertext` for `gallery_id`, failing if the key, nonce, tag, or
    /// gallery AAD does not match.
    pub fn decrypt(
        &self,
        gallery_id: GalleryId,
        ciphertext: &EncryptedAccessCode,
    ) -> Result<GalleryAccessCode, AccessCodeCryptoError> {
        let bytes = ciphertext.as_array().map_err(|_| AccessCodeCryptoError)?;
        let nonce_bytes: [u8; NONCE_BYTES] = bytes[..NONCE_BYTES]
            .try_into()
            .map_err(|_| AccessCodeCryptoError)?;
        let nonce = Nonce::from(nonce_bytes);
        let payload = Payload {
            msg: &bytes[NONCE_BYTES..],
            aad: gallery_id.0.as_bytes(),
        };
        let plaintext = self
            .aead
            .decrypt(&nonce, payload)
            .map_err(|_| AccessCodeCryptoError)?;
        let text = String::from_utf8(plaintext).map_err(|_| AccessCodeCryptoError)?;

        text.parse().map_err(|_| AccessCodeCryptoError)
    }

    /// Decrypts `ciphertext` for `gallery_id` and compares it with `submitted` in constant time.
    pub fn verify_code(
        &self,
        gallery_id: GalleryId,
        ciphertext: &EncryptedAccessCode,
        submitted: &GalleryAccessCode,
    ) -> Result<bool, AccessCodeCryptoError> {
        let expected = self.decrypt(gallery_id, ciphertext)?;

        Ok(bool::from(expected.as_bytes().ct_eq(submitted.as_bytes())))
    }

    /// Whether `cookie_value` carries a valid HMAC-authenticated unlock entry for
    /// `(gallery_id, sha256(ciphertext))`.
    pub fn is_unlocked(
        &self,
        cookie_value: Option<&str>,
        gallery_id: GalleryId,
        ciphertext: &EncryptedAccessCode,
    ) -> bool {
        let Some(cookie_value) = cookie_value else {
            return false;
        };
        let Some(records) = self.verified_records(cookie_value) else {
            return false;
        };
        let expected_digest = Self::ciphertext_digest(ciphertext);

        records.iter().any(|record| {
            record.gallery_id == gallery_id && bool::from(record.digest.ct_eq(&expected_digest))
        })
    }

    /// Produces an updated base64url HMAC-signed cookie value recording `(gallery_id, ciphertext)`
    /// while preserving up to `MAX_UNLOCKED_GALLERIES - 1` previously unlocked galleries.
    pub fn issue_unlock_cookie_value(
        &self,
        existing_cookie_value: Option<&str>,
        gallery_id: GalleryId,
        ciphertext: &EncryptedAccessCode,
    ) -> String {
        let mut records = existing_cookie_value
            .and_then(|value| self.verified_records(value))
            .unwrap_or_default();

        records.retain(|record| record.gallery_id != gallery_id);
        records.push(UnlockRecord {
            gallery_id,
            digest: Self::ciphertext_digest(ciphertext),
        });

        if records.len() > MAX_UNLOCKED_GALLERIES {
            let overflow = records.len() - MAX_UNLOCKED_GALLERIES;

            records.drain(..overflow);
        }

        let mut payload =
            Vec::with_capacity(1 + (records.len() * UNLOCK_RECORD_BYTES) + DIGEST_BYTES);

        payload.push(COOKIE_VERSION);

        for record in &records {
            record.append_to(&mut payload);
        }

        let tag = self.compute_cookie_mac(&payload);

        payload.extend_from_slice(&tag);

        URL_SAFE_NO_PAD.encode(payload)
    }

    fn hmac(key: &[u8; KEY_BYTES]) -> HmacSha256 {
        // RFC 2104 HMAC-SHA256 accepts keys of every byte length; 32-byte keys cannot fail.
        #[allow(clippy::expect_used)]
        HmacSha256::new_from_slice(key).expect("32-byte HMAC-SHA256 key is always valid")
    }

    fn derive_subkey(master_key: &[u8; KEY_BYTES], label: &[u8]) -> [u8; KEY_BYTES] {
        let mut mac = Self::hmac(master_key);

        mac.update(label);

        mac.finalize().into_bytes().into()
    }

    fn ciphertext_digest(ciphertext: &EncryptedAccessCode) -> [u8; DIGEST_BYTES] {
        Sha256::digest(ciphertext.as_bytes()).into()
    }

    fn compute_cookie_mac(&self, payload: &[u8]) -> [u8; DIGEST_BYTES] {
        let mut mac = Self::hmac(&self.cookie_key);

        mac.update(payload);

        mac.finalize().into_bytes().into()
    }

    fn verified_records(&self, encoded: &str) -> Option<Vec<UnlockRecord>> {
        if encoded.is_empty() || encoded.len() > MAX_COOKIE_ENCODED_LEN {
            return None;
        }

        let raw = URL_SAFE_NO_PAD.decode(encoded).ok()?;

        if !(MIN_COOKIE_RAW_BYTES..=MAX_COOKIE_RAW_BYTES).contains(&raw.len()) {
            return None;
        }

        let payload_len = raw.len() - DIGEST_BYTES;
        let (payload, tag) = raw.split_at(payload_len);

        if payload.first().copied() != Some(COOKIE_VERSION) {
            return None;
        }

        let (records, remainder) = payload[1..].as_chunks::<UNLOCK_RECORD_BYTES>();

        if !remainder.is_empty() {
            return None;
        }

        let mut mac = Self::hmac(&self.cookie_key);

        mac.update(payload);
        mac.verify_slice(tag).ok()?;

        Some(records.iter().map(UnlockRecord::from_bytes).collect())
    }
}

impl fmt::Debug for AccessCodeCipher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessCodeCipher(..)")
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use pp_core::types::generate::Generate;

    use super::*;

    fn sample_key(byte: u8) -> EncryptionKey {
        EncryptionKey::from_bytes([byte; KEY_BYTES])
    }

    #[test]
    fn encryption_key_parses_hex_and_canonical_base64url_and_redacts_debug() {
        let bytes = [0x5au8; KEY_BYTES];
        let hex_lower = "5a".repeat(KEY_BYTES);
        let hex_upper = "5A".repeat(KEY_BYTES);
        let b64 = URL_SAFE_NO_PAD.encode(bytes);

        assert_eq!(b64.len(), 43);
        assert_eq!(
            hex_lower.parse::<EncryptionKey>().unwrap().as_bytes(),
            &bytes
        );
        assert_eq!(
            hex_upper.parse::<EncryptionKey>().unwrap().as_bytes(),
            &bytes
        );
        assert_eq!(b64.parse::<EncryptionKey>().unwrap().as_bytes(), &bytes);

        let key = hex_lower.parse::<EncryptionKey>().unwrap();
        let cipher = AccessCodeCipher::new(&key);

        assert_eq!(format!("{key:?}"), "EncryptionKey(..)");
        assert_eq!(format!("{cipher:?}"), "AccessCodeCipher(..)");

        let non_canonical_b64 = format!("{}B", "A".repeat(42));

        for invalid in [
            "",
            "short",
            &"5a".repeat(31),
            &"5a".repeat(33),
            &"zz".repeat(32),
            &format!("{}=", URL_SAFE_NO_PAD.encode(bytes)),
            non_canonical_b64.as_str(),
        ] {
            let err = invalid.parse::<EncryptionKey>().unwrap_err();

            if !invalid.is_empty() {
                assert!(!err.to_string().contains(invalid), "{invalid:?}");
            }
        }
    }

    #[test]
    fn cipher_round_trips_and_binds_ciphertext_to_gallery_id_and_key() {
        let cipher = AccessCodeCipher::new(&sample_key(0x11));
        let other_cipher = AccessCodeCipher::new(&sample_key(0x22));
        let gallery_a = GalleryId::generate();
        let gallery_b = GalleryId::generate();
        let code: GalleryAccessCode = "042819".parse().unwrap();
        let wrong_code: GalleryAccessCode = "042818".parse().unwrap();

        let first = cipher.encrypt(gallery_a, &code).unwrap();
        let second = cipher.encrypt(gallery_a, &code).unwrap();

        assert_eq!(first.as_bytes().len(), 34);
        assert_ne!(
            first, second,
            "random 96-bit nonce must vary per encryption"
        );
        assert_eq!(cipher.decrypt(gallery_a, &first).unwrap(), code);
        assert_eq!(cipher.decrypt(gallery_a, &second).unwrap(), code);
        assert!(cipher.verify_code(gallery_a, &first, &code).unwrap());
        assert!(!cipher.verify_code(gallery_a, &first, &wrong_code).unwrap());

        // Transplanting ciphertext to another gallery must fail AAD authentication.
        assert!(cipher.decrypt(gallery_b, &first).is_err());
        assert!(cipher.verify_code(gallery_b, &first, &code).is_err());

        // Decrypting with a different key or tampered byte must fail.
        assert!(other_cipher.decrypt(gallery_a, &first).is_err());

        let mut tampered = *first.as_array().unwrap();
        tampered[15] ^= 0x01;
        let tampered = EncryptedAccessCode::from(tampered);

        assert!(cipher.decrypt(gallery_a, &tampered).is_err());
    }

    #[test]
    fn unlock_cookie_supports_multiple_galleries_caps_at_sixteen_and_rejects_tampering() {
        let cipher = AccessCodeCipher::new(&sample_key(0x33));
        let other_cipher = AccessCodeCipher::new(&sample_key(0x44));
        let code: GalleryAccessCode = "901234".parse().unwrap();

        let mut galleries = Vec::new();
        let mut cookie: Option<String> = None;

        for _ in 0..18 {
            let id = GalleryId::generate();
            let encrypted = cipher.encrypt(id, &code).unwrap();
            let next = cipher.issue_unlock_cookie_value(cookie.as_deref(), id, &encrypted);

            assert!(!next.contains("901234"));
            assert!(cipher.is_unlocked(Some(&next), id, &encrypted));
            cookie = Some(next);
            galleries.push((id, encrypted));
        }

        let cookie = cookie.unwrap();

        // Oldest two entries (0 and 1) were evicted when entries 16 and 17 were added; 2..18 stay unlocked.
        assert!(!cipher.is_unlocked(Some(&cookie), galleries[0].0, &galleries[0].1));
        assert!(!cipher.is_unlocked(Some(&cookie), galleries[1].0, &galleries[1].1));

        for (id, encrypted) in &galleries[2..18] {
            assert!(cipher.is_unlocked(Some(&cookie), *id, encrypted));
            assert!(!other_cipher.is_unlocked(Some(&cookie), *id, encrypted));
        }

        // Re-unlocking gallery[2] refreshes its recency without duplicating, evicting gallery[3] only when a new 19th arrives.
        let refreshed =
            cipher.issue_unlock_cookie_value(Some(&cookie), galleries[2].0, &galleries[2].1);
        let extra_id = GalleryId::generate();
        let extra_enc = cipher.encrypt(extra_id, &code).unwrap();
        let after_extra = cipher.issue_unlock_cookie_value(Some(&refreshed), extra_id, &extra_enc);

        assert!(cipher.is_unlocked(Some(&after_extra), galleries[2].0, &galleries[2].1));
        assert!(!cipher.is_unlocked(Some(&after_extra), galleries[3].0, &galleries[3].1));
        assert!(cipher.is_unlocked(Some(&after_extra), extra_id, &extra_enc));

        // Different ciphertext for the same gallery_id is NOT unlocked by the old cookie entry.
        let rotated_enc = cipher.encrypt(extra_id, &code).unwrap();
        assert!(!cipher.is_unlocked(Some(&after_extra), extra_id, &rotated_enc));

        // Tampered, empty, oversized, or garbage cookie values fail closed and are replaced cleanly on next unlock.
        let mut raw = URL_SAFE_NO_PAD.decode(&after_extra).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0x01;
        let forged = URL_SAFE_NO_PAD.encode(raw);

        assert!(!cipher.is_unlocked(Some(&forged), extra_id, &extra_enc));
        assert!(!cipher.is_unlocked(None, extra_id, &extra_enc));
        assert!(!cipher.is_unlocked(Some(""), extra_id, &extra_enc));
        assert!(!cipher.is_unlocked(Some(&"A".repeat(2000)), extra_id, &extra_enc));

        let recovered = cipher.issue_unlock_cookie_value(Some(&forged), extra_id, &extra_enc);
        assert!(cipher.is_unlocked(Some(&recovered), extra_id, &extra_enc));
    }
}
