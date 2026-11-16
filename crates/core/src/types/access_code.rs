//! Per-gallery 6-digit access codes and stateless `pp_gallery_unlock` cookie signing.
//!
//! [`GalleryAccessCode`] validates the 6 ASCII digits (`000000`–`999999`) stored in PostgreSQL
//! and compares submitted codes in constant time. [`UnlockCookieSigner`] holds a random 256-bit
//! in-memory HMAC-SHA256 key generated when `pp-api` starts, authenticating up to
//! [`MAX_UNLOCKED_GALLERIES`] `(gallery_id, code)` entries in the `HttpOnly`
//! [`UNLOCK_COOKIE_NAME`] cookie without storing client unlock sessions in the database.
//!
//! None of the types in this module prints raw key material, digits, or cookie tokens in
//! [`fmt::Debug`].

use std::{fmt, str::FromStr};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit as _, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize, Serializer};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

/// Length of a gallery access code in ASCII digits.
pub const ACCESS_CODE_DIGITS: usize = 6;

/// Name of the `HttpOnly` cookie recording unlocked galleries in a client browser.
pub const UNLOCK_COOKIE_NAME: &str = "pp_gallery_unlock";

/// Maximum `(gallery_id, tag)` entries retained in [`UNLOCK_COOKIE_NAME`].
pub const MAX_UNLOCKED_GALLERIES: usize = 16;

const KEY_BYTES: usize = 32;
const TAG_BYTES: usize = 32;
const UUID_BYTES: usize = 16;
const UNLOCK_RECORD_BYTES: usize = UUID_BYTES + TAG_BYTES;
const MAX_COOKIE_RAW_BYTES: usize = MAX_UNLOCKED_GALLERIES * UNLOCK_RECORD_BYTES;
const MAX_COOKIE_ENCODED_LEN: usize = (MAX_COOKIE_RAW_BYTES * 8).div_ceil(6);

type HmacSha256 = Hmac<Sha256>;

/// The input is not exactly 6 ASCII digits (`000000`–`999999`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("access code must be 6 ASCII digits")]
pub struct ParseAccessCodeError;

/// Six-digit numeric code (`000000`–`999999`) gating client access to a shared gallery.
///
/// [`Debug`](fmt::Debug) redacts the digits so credentials never leak into logs or spans.
///
/// ```
/// use pp_core::types::access_code::GalleryAccessCode;
///
/// let code: GalleryAccessCode = "042819".parse().unwrap();
/// assert_eq!(code.as_str(), "042819");
/// assert!(code.verify(&"042819".parse().unwrap()));
/// assert_eq!(format!("{code:?}"), "GalleryAccessCode(..)");
/// assert!("12345".parse::<GalleryAccessCode>().is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct GalleryAccessCode(String);

impl GalleryAccessCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Compares `self` with `submitted` in constant time.
    pub fn verify(&self, submitted: &Self) -> bool {
        bool::from(self.as_bytes().ct_eq(submitted.as_bytes()))
    }

    /// Serializes only whether an access code is present (`true` or `false`), keeping raw digits
    /// out of dashboard overview responses.
    pub fn serialize_presence<S>(value: &Option<Self>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(value.is_some())
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

/// Fixed-width `(gallery_id, hmac_tag)` entry inside [`UNLOCK_COOKIE_NAME`].
#[derive(Clone, Copy)]
struct UnlockRecord {
    gallery_id: Uuid,
    tag: [u8; TAG_BYTES],
}

impl UnlockRecord {
    fn from_bytes(record: &[u8; UNLOCK_RECORD_BYTES]) -> Self {
        let mut uuid_bytes = [0u8; UUID_BYTES];
        let mut tag = [0u8; TAG_BYTES];

        uuid_bytes.copy_from_slice(&record[..UUID_BYTES]);
        tag.copy_from_slice(&record[UUID_BYTES..]);

        Self {
            gallery_id: Uuid::from_bytes(uuid_bytes),
            tag,
        }
    }

    fn append_to(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(self.gallery_id.as_bytes());
        buffer.extend_from_slice(&self.tag);
    }
}

/// In-memory HMAC-SHA256 signer and verifier for [`UNLOCK_COOKIE_NAME`].
///
/// [`Debug`](fmt::Debug) redacts the signing key so state dumps never leak secret bytes.
///
/// ```
/// use pp_core::types::access_code::{GalleryAccessCode, UnlockCookieSigner};
/// use uuid::Uuid;
///
/// let signer = UnlockCookieSigner::generate();
/// let gallery_id = Uuid::new_v4();
/// let code: GalleryAccessCode = "042819".parse().unwrap();
/// let cookie = signer.issue_unlock_cookie_value(None, gallery_id, &code);
///
/// assert!(signer.is_unlocked(Some(&cookie), gallery_id, &code));
/// assert_eq!(format!("{signer:?}"), "UnlockCookieSigner(..)");
/// ```
#[derive(Clone)]
pub struct UnlockCookieSigner {
    key: [u8; KEY_BYTES],
}

impl UnlockCookieSigner {
    /// Generates a fresh random 256-bit signing key from the OS CSPRNG.
    pub fn generate() -> Self {
        let mut key = [0u8; KEY_BYTES];

        rand::rng().fill_bytes(&mut key);

        Self { key }
    }

    /// Builds a signer from explicit key bytes (for deterministic tests).
    pub const fn from_bytes(key: [u8; KEY_BYTES]) -> Self {
        Self { key }
    }

    /// Whether `cookie_value` carries a valid HMAC-authenticated unlock entry for
    /// `(gallery_id, code)`.
    pub fn is_unlocked(
        &self,
        cookie_value: Option<&str>,
        gallery_id: impl Into<Uuid>,
        code: &GalleryAccessCode,
    ) -> bool {
        let gallery_id = gallery_id.into();
        let Some(cookie_value) = cookie_value else {
            return false;
        };
        let Some(records) = Self::decode_records(cookie_value) else {
            return false;
        };

        records.iter().any(|record| {
            record.gallery_id == gallery_id && self.verify_tag(gallery_id, code, &record.tag)
        })
    }

    /// Produces an updated base64url cookie value recording `(gallery_id, code)` while preserving
    /// up to `MAX_UNLOCKED_GALLERIES - 1` previously unlocked galleries.
    pub fn issue_unlock_cookie_value(
        &self,
        existing_cookie_value: Option<&str>,
        gallery_id: impl Into<Uuid>,
        code: &GalleryAccessCode,
    ) -> String {
        let gallery_id = gallery_id.into();
        let mut records = existing_cookie_value
            .and_then(Self::decode_records)
            .unwrap_or_default();

        records.retain(|record| record.gallery_id != gallery_id);
        records.push(UnlockRecord {
            gallery_id,
            tag: self.sign_tag(gallery_id, code),
        });

        if records.len() > MAX_UNLOCKED_GALLERIES {
            let overflow = records.len() - MAX_UNLOCKED_GALLERIES;

            records.drain(..overflow);
        }

        let mut payload = Vec::with_capacity(records.len() * UNLOCK_RECORD_BYTES);

        for record in &records {
            record.append_to(&mut payload);
        }

        URL_SAFE_NO_PAD.encode(payload)
    }

    fn mac(&self, gallery_id: Uuid, code: &GalleryAccessCode) -> HmacSha256 {
        // RFC 2104 HMAC-SHA256 accepts keys of every byte length; 32-byte keys cannot fail.
        #[allow(clippy::expect_used)]
        let mut mac =
            HmacSha256::new_from_slice(&self.key).expect("32-byte HMAC-SHA256 key is always valid");

        mac.update(gallery_id.as_bytes());
        mac.update(code.as_bytes());

        mac
    }

    fn sign_tag(&self, gallery_id: Uuid, code: &GalleryAccessCode) -> [u8; TAG_BYTES] {
        self.mac(gallery_id, code).finalize().into_bytes().into()
    }

    fn verify_tag(
        &self,
        gallery_id: Uuid,
        code: &GalleryAccessCode,
        tag: &[u8; TAG_BYTES],
    ) -> bool {
        self.mac(gallery_id, code).verify_slice(tag).is_ok()
    }

    fn decode_records(encoded: &str) -> Option<Vec<UnlockRecord>> {
        if encoded.is_empty() || encoded.len() > MAX_COOKIE_ENCODED_LEN {
            return None;
        }

        let raw = URL_SAFE_NO_PAD.decode(encoded).ok()?;

        if raw.is_empty()
            || raw.len() > MAX_COOKIE_RAW_BYTES
            || URL_SAFE_NO_PAD.encode(&raw) != encoded
        {
            return None;
        }

        let (records, remainder) = raw.as_chunks::<UNLOCK_RECORD_BYTES>();

        if !remainder.is_empty() {
            return None;
        }

        Some(records.iter().map(UnlockRecord::from_bytes).collect())
    }
}

impl fmt::Debug for UnlockCookieSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("UnlockCookieSigner(..)")
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::json;

    use super::*;

    #[derive(Serialize)]
    struct PresenceHolder {
        #[serde(serialize_with = "GalleryAccessCode::serialize_presence")]
        protected: Option<GalleryAccessCode>,
    }

    #[test]
    fn accepts_only_six_ascii_digits_and_verifies_in_constant_time() {
        for valid in ["000000", "042819", "123456", "999999"] {
            let parsed: GalleryAccessCode = valid.parse().expect(valid);
            let other: GalleryAccessCode = "654321".parse().unwrap();

            assert_eq!(parsed.as_str(), valid);
            assert_eq!(parsed.as_bytes(), valid.as_bytes());
            assert!(parsed.verify(&parsed));
            assert_eq!(parsed.verify(&other), valid == "654321");
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
    fn debug_redacts_digits_and_serialize_presence_emits_only_boolean() {
        let code: GalleryAccessCode = "042819".parse().unwrap();

        assert_eq!(format!("{code:?}"), "GalleryAccessCode(..)");
        assert!(!format!("{code:?}").contains("042819"));

        let some_json = serde_json::to_value(PresenceHolder {
            protected: Some(code),
        })
        .unwrap();
        let none_json = serde_json::to_value(PresenceHolder { protected: None }).unwrap();

        assert_eq!(some_json, json!({ "protected": true }));
        assert_eq!(none_json, json!({ "protected": false }));
    }

    #[test]
    fn unlock_cookie_signer_binds_gallery_and_code_caps_at_sixteen_and_rejects_tampering() {
        let signer = UnlockCookieSigner::from_bytes([0x33; KEY_BYTES]);
        let other_signer = UnlockCookieSigner::from_bytes([0x44; KEY_BYTES]);
        let generated = UnlockCookieSigner::generate();
        let code: GalleryAccessCode = "901234".parse().unwrap();
        let rotated_code: GalleryAccessCode = "901235".parse().unwrap();

        assert_eq!(format!("{signer:?}"), "UnlockCookieSigner(..)");
        assert_eq!(format!("{generated:?}"), "UnlockCookieSigner(..)");

        let gallery_a = Uuid::new_v4();
        let gallery_b = Uuid::new_v4();
        let cookie_a = signer.issue_unlock_cookie_value(None, gallery_a, &code);

        assert!(signer.is_unlocked(Some(&cookie_a), gallery_a, &code));
        assert!(
            !signer.is_unlocked(Some(&cookie_a), gallery_b, &code),
            "identical PIN on another gallery must not unlock via gallery_a's cookie"
        );
        assert!(!signer.is_unlocked(Some(&cookie_a), gallery_a, &rotated_code));
        assert!(!other_signer.is_unlocked(Some(&cookie_a), gallery_a, &code));

        let mut galleries = Vec::new();
        let mut cookie: Option<String> = None;

        for _ in 0..18 {
            let id = Uuid::new_v4();
            let next = signer.issue_unlock_cookie_value(cookie.as_deref(), id, &code);

            assert!(!next.contains("901234"));
            assert!(signer.is_unlocked(Some(&next), id, &code));
            cookie = Some(next);
            galleries.push(id);
        }

        let cookie = cookie.unwrap();

        assert!(!signer.is_unlocked(Some(&cookie), galleries[0], &code));
        assert!(!signer.is_unlocked(Some(&cookie), galleries[1], &code));

        for id in &galleries[2..18] {
            assert!(signer.is_unlocked(Some(&cookie), *id, &code));
            assert!(!other_signer.is_unlocked(Some(&cookie), *id, &code));
        }

        let refreshed = signer.issue_unlock_cookie_value(Some(&cookie), galleries[2], &code);
        let extra_id = Uuid::new_v4();
        let after_extra = signer.issue_unlock_cookie_value(Some(&refreshed), extra_id, &code);

        assert!(signer.is_unlocked(Some(&after_extra), galleries[2], &code));
        assert!(!signer.is_unlocked(Some(&after_extra), galleries[3], &code));
        assert!(signer.is_unlocked(Some(&after_extra), extra_id, &code));

        let mut raw = URL_SAFE_NO_PAD.decode(&after_extra).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0x01;
        let forged_last = URL_SAFE_NO_PAD.encode(&raw);

        assert!(!signer.is_unlocked(Some(&forged_last), extra_id, &code));
        assert!(signer.is_unlocked(Some(&forged_last), galleries[2], &code));

        let padded = format!("{cookie_a}=");
        let non_canonical = format!("{}B", "A".repeat(63));
        let wrong_len = URL_SAFE_NO_PAD.encode([0x11u8; 47]);
        let oversized = URL_SAFE_NO_PAD.encode(vec![0x11u8; 17 * 48]);

        for invalid in [
            None,
            Some(""),
            Some(padded.as_str()),
            Some(non_canonical.as_str()),
            Some(wrong_len.as_str()),
            Some(oversized.as_str()),
        ] {
            assert!(
                !signer.is_unlocked(invalid, gallery_a, &code),
                "{invalid:?}"
            );
        }

        let recovered = signer.issue_unlock_cookie_value(Some(&wrong_len), extra_id, &code);

        assert!(signer.is_unlocked(Some(&recovered), extra_id, &code));
    }
}
