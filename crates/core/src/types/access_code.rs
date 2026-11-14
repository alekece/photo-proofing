//! Per-gallery 6-digit access codes, AEAD encryption at rest, and stateless unlock cookies.
//!
//! [`AccessCodeCipher`] derives two domain-separated 256-bit subkeys via HMAC-SHA256 from the
//! operator's [`EncryptionKey`]:
//! - a `ChaCha20-Poly1305` AEAD subkey that encrypts the 6 ASCII digits of a
//!   [`GalleryAccessCode`] with a random 96-bit nonce and `gallery_id` as AAD (`34` bytes);
//! - an `HMAC-SHA256` subkey that signs and verifies the `pp_gallery_unlock` `HttpOnly` cookie,
//!   binding up to [`MAX_UNLOCKED_GALLERIES`] `(gallery_id, sha256(ciphertext))` entries.
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
use rand::Rng;
use serde::{Deserialize, Serialize, Serializer};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

/// Length of a gallery access code in ASCII digits.
pub const ACCESS_CODE_DIGITS: usize = 6;

/// Size in bytes of an AEAD-encrypted gallery access code (`12`-byte nonce + `6`-byte ASCII
/// digits + `16`-byte authentication tag).
pub const ENCRYPTED_ACCESS_CODE_LEN: usize = 34;

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

/// The input is not exactly 6 ASCII digits (`000000`–`999999`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("access code must be 6 ASCII digits")]
pub struct ParseAccessCodeError;

/// Ciphertext length did not match [`ENCRYPTED_ACCESS_CODE_LEN`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("encrypted access code must be {ENCRYPTED_ACCESS_CODE_LEN} bytes")]
pub struct EncryptedAccessCodeError;

/// The configured key is neither 64 hex digits nor canonical 43-char unpadded base64url.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("encryption key must be 32 bytes as 64 hex digits or 43 unpadded base64url characters")]
pub struct ParseEncryptionKeyError;

/// Encryption or decryption of a gallery access code failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("gallery access code cipher failed")]
pub struct AccessCodeCryptoError;

/// Six-digit numeric code (`000000`–`999999`) gating client access to a shared gallery.
///
/// [`Debug`](fmt::Debug) redacts the digits so credentials never leak into logs or spans.
///
/// ```
/// use pp_core::types::access_code::GalleryAccessCode;
///
/// let code: GalleryAccessCode = "042819".parse().unwrap();
/// assert_eq!(code.as_str(), "042819");
/// assert_eq!(format!("{code:?}"), "GalleryAccessCode(..)");
/// assert!("12345".parse::<GalleryAccessCode>().is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GalleryAccessCode(String);

impl GalleryAccessCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for GalleryAccessCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GalleryAccessCode(..)")
    }
}

impl FromStr for GalleryAccessCode {
    type Err = ParseAccessCodeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != ACCESS_CODE_DIGITS || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ParseAccessCodeError);
        }

        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for GalleryAccessCode {
    type Error = ParseAccessCodeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<GalleryAccessCode> for String {
    fn from(code: GalleryAccessCode) -> Self {
        code.0
    }
}

/// AEAD-encrypted 6-digit gallery access code stored in PostgreSQL `bytea`.
///
/// [`Debug`](fmt::Debug) redacts the ciphertext bytes, and neither `Serialize` nor `Deserialize`
/// is implemented so raw ciphertext can never cross a JSON boundary.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct EncryptedAccessCode(Vec<u8>);

impl EncryptedAccessCode {
    pub fn from_array(bytes: [u8; ENCRYPTED_ACCESS_CODE_LEN]) -> Self {
        Self(bytes.to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn as_array(&self) -> Result<&[u8; ENCRYPTED_ACCESS_CODE_LEN], EncryptedAccessCodeError> {
        self.0
            .as_slice()
            .try_into()
            .map_err(|_| EncryptedAccessCodeError)
    }

    pub fn serialize_presence<S>(value: &Option<Self>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(value.is_some())
    }
}

impl fmt::Debug for EncryptedAccessCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("EncryptedAccessCode(..)")
    }
}

impl From<[u8; ENCRYPTED_ACCESS_CODE_LEN]> for EncryptedAccessCode {
    fn from(bytes: [u8; ENCRYPTED_ACCESS_CODE_LEN]) -> Self {
        Self::from_array(bytes)
    }
}

impl TryFrom<Vec<u8>> for EncryptedAccessCode {
    type Error = EncryptedAccessCodeError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != ENCRYPTED_ACCESS_CODE_LEN {
            return Err(EncryptedAccessCodeError);
        }

        Ok(Self(bytes))
    }
}

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
    pub fn encode_base64url(self) -> String {
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
    gallery_id: Uuid,
    digest: [u8; DIGEST_BYTES],
}

impl UnlockRecord {
    fn from_bytes(record: &[u8; UNLOCK_RECORD_BYTES]) -> Self {
        let mut uuid_bytes = [0u8; UUID_BYTES];
        let mut digest = [0u8; DIGEST_BYTES];

        uuid_bytes.copy_from_slice(&record[..UUID_BYTES]);
        digest.copy_from_slice(&record[UUID_BYTES..]);

        Self {
            gallery_id: Uuid::from_bytes(uuid_bytes),
            digest,
        }
    }

    fn append_to(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.gallery_id.as_bytes());
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
        gallery_id: impl Into<Uuid>,
        code: &GalleryAccessCode,
    ) -> Result<EncryptedAccessCode, AccessCodeCryptoError> {
        let gallery_id = gallery_id.into();
        let mut nonce_bytes = [0u8; NONCE_BYTES];

        rand::rng().fill_bytes(&mut nonce_bytes);

        let nonce = Nonce::from(nonce_bytes);
        let payload = Payload {
            msg: code.as_bytes(),
            aad: gallery_id.as_bytes(),
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
        gallery_id: impl Into<Uuid>,
        ciphertext: &EncryptedAccessCode,
    ) -> Result<GalleryAccessCode, AccessCodeCryptoError> {
        let gallery_id = gallery_id.into();
        let bytes = ciphertext.as_array().map_err(|_| AccessCodeCryptoError)?;
        let nonce_bytes: [u8; NONCE_BYTES] = bytes[..NONCE_BYTES]
            .try_into()
            .map_err(|_| AccessCodeCryptoError)?;
        let nonce = Nonce::from(nonce_bytes);
        let payload = Payload {
            msg: &bytes[NONCE_BYTES..],
            aad: gallery_id.as_bytes(),
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
        gallery_id: impl Into<Uuid>,
        ciphertext: &EncryptedAccessCode,
        submitted: &GalleryAccessCode,
    ) -> Result<bool, AccessCodeCryptoError> {
        let expected = self.decrypt(gallery_id, ciphertext)?;

        Ok(bool::from(expected.as_bytes().ct_eq(submitted.as_bytes())))
    }

    /// Signs `message` with the domain-separated cookie HMAC-SHA256 subkey.
    pub fn sign_cookie_bytes(&self, message: &[u8]) -> [u8; DIGEST_BYTES] {
        let mut mac = Self::hmac(&self.cookie_key);

        mac.update(message);

        mac.finalize().into_bytes().into()
    }

    /// Verifies `tag` over `message` in constant time using the cookie HMAC-SHA256 subkey.
    pub fn verify_cookie_bytes(&self, message: &[u8], tag: &[u8]) -> bool {
        let mut mac = Self::hmac(&self.cookie_key);

        mac.update(message);

        mac.verify_slice(tag).is_ok()
    }

    /// Whether `cookie_value` carries a valid HMAC-authenticated unlock entry for
    /// `(gallery_id, sha256(ciphertext))`.
    pub fn is_unlocked(
        &self,
        cookie_value: Option<&str>,
        gallery_id: impl Into<Uuid>,
        ciphertext: &EncryptedAccessCode,
    ) -> bool {
        let gallery_id = gallery_id.into();
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
        gallery_id: impl Into<Uuid>,
        ciphertext: &EncryptedAccessCode,
    ) -> String {
        let gallery_id = gallery_id.into();
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

        let tag = self.sign_cookie_bytes(&payload);

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

        if !self.verify_cookie_bytes(payload, tag) {
            return None;
        }

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
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_only_six_ascii_digits() {
        for valid in ["000000", "042819", "123456", "999999"] {
            let parsed: GalleryAccessCode = valid.parse().expect(valid);

            assert_eq!(parsed.as_str(), valid);
            assert_eq!(parsed.as_bytes(), valid.as_bytes());
            assert_eq!(serde_json::to_value(&parsed).unwrap(), json!(valid));
            assert_eq!(
                serde_json::from_value::<GalleryAccessCode>(json!(valid)).unwrap(),
                parsed
            );
        }

        for invalid in [
            "",
            "0",
            "12345",
            "1234567",
            " 123456",
            "123456 ",
            "12 345",
            "+12345",
            "-12345",
            "abcdef",
            "12345a",
            "١٢٣٤٥٦",
        ] {
            let err = invalid.parse::<GalleryAccessCode>().unwrap_err();

            if !invalid.is_empty() {
                assert!(!err.to_string().contains(invalid), "{invalid:?}");
            }
            assert!(
                serde_json::from_value::<GalleryAccessCode>(json!(invalid)).is_err(),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn debug_redacts_the_digits() {
        let code: GalleryAccessCode = "042819".parse().unwrap();

        assert_eq!(format!("{code:?}"), "GalleryAccessCode(..)");
        assert!(!format!("{code:?}").contains("042819"));
    }

    #[test]
    fn cipher_round_trips_and_binds_gallery_uuid_aad() {
        let key = EncryptionKey::from_bytes([0x42; KEY_BYTES]);
        let cipher = AccessCodeCipher::new(&key);
        let gallery_id = uuid::Uuid::new_v4();
        let other_gallery_id = uuid::Uuid::new_v4();
        let code: GalleryAccessCode = "042819".parse().unwrap();

        let encrypted = cipher.encrypt(gallery_id, &code).unwrap();

        assert_eq!(encrypted.as_bytes().len(), ENCRYPTED_ACCESS_CODE_LEN);
        assert_eq!(format!("{key:?}"), "EncryptionKey(..)");
        assert_eq!(format!("{encrypted:?}"), "EncryptedAccessCode(..)");
        assert_eq!(format!("{cipher:?}"), "AccessCodeCipher(..)");
        assert_eq!(cipher.decrypt(gallery_id, &encrypted).unwrap(), code);
        assert!(cipher.decrypt(other_gallery_id, &encrypted).is_err());

        let tag = cipher.sign_cookie_bytes(b"cookie-payload");

        assert!(cipher.verify_cookie_bytes(b"cookie-payload", &tag));
        assert!(!cipher.verify_cookie_bytes(b"tampered", &tag));

        let wrong_code: GalleryAccessCode = "042818".parse().unwrap();
        let other_cipher = AccessCodeCipher::new(&EncryptionKey::from_bytes([0x22; KEY_BYTES]));
        let second = cipher.encrypt(gallery_id, &code).unwrap();

        assert_ne!(
            encrypted, second,
            "random 96-bit nonce must vary per encryption"
        );
        assert!(cipher.verify_code(gallery_id, &encrypted, &code).unwrap());
        assert!(
            !cipher
                .verify_code(gallery_id, &encrypted, &wrong_code)
                .unwrap()
        );
        assert!(
            cipher
                .verify_code(other_gallery_id, &encrypted, &code)
                .is_err()
        );
        assert!(other_cipher.decrypt(gallery_id, &encrypted).is_err());

        let mut tampered = *encrypted.as_array().unwrap();
        tampered[15] ^= 0x01;
        let tampered = EncryptedAccessCode::from(tampered);

        assert!(cipher.decrypt(gallery_id, &tampered).is_err());
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
    fn unlock_cookie_supports_multiple_galleries_caps_at_sixteen_and_rejects_tampering() {
        let cipher = AccessCodeCipher::new(&EncryptionKey::from_bytes([0x33; KEY_BYTES]));
        let other_cipher = AccessCodeCipher::new(&EncryptionKey::from_bytes([0x44; KEY_BYTES]));
        let code: GalleryAccessCode = "901234".parse().unwrap();

        let mut galleries = Vec::new();
        let mut cookie: Option<String> = None;

        for _ in 0..18 {
            let id = Uuid::new_v4();
            let encrypted = cipher.encrypt(id, &code).unwrap();
            let next = cipher.issue_unlock_cookie_value(cookie.as_deref(), id, &encrypted);

            assert!(!next.contains("901234"));
            assert!(cipher.is_unlocked(Some(&next), id, &encrypted));
            cookie = Some(next);
            galleries.push((id, encrypted));
        }

        let cookie = cookie.unwrap();

        assert!(!cipher.is_unlocked(Some(&cookie), galleries[0].0, &galleries[0].1));
        assert!(!cipher.is_unlocked(Some(&cookie), galleries[1].0, &galleries[1].1));

        for (id, encrypted) in &galleries[2..18] {
            assert!(cipher.is_unlocked(Some(&cookie), *id, encrypted));
            assert!(!other_cipher.is_unlocked(Some(&cookie), *id, encrypted));
        }

        let refreshed =
            cipher.issue_unlock_cookie_value(Some(&cookie), galleries[2].0, &galleries[2].1);
        let extra_id = Uuid::new_v4();
        let extra_enc = cipher.encrypt(extra_id, &code).unwrap();
        let after_extra = cipher.issue_unlock_cookie_value(Some(&refreshed), extra_id, &extra_enc);

        assert!(cipher.is_unlocked(Some(&after_extra), galleries[2].0, &galleries[2].1));
        assert!(!cipher.is_unlocked(Some(&after_extra), galleries[3].0, &galleries[3].1));
        assert!(cipher.is_unlocked(Some(&after_extra), extra_id, &extra_enc));

        let rotated_enc = cipher.encrypt(extra_id, &code).unwrap();

        assert!(!cipher.is_unlocked(Some(&after_extra), extra_id, &rotated_enc));

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
