//! Per-gallery 6-digit access codes and stateless `pp_gallery_unlock` cookie signing.
//!
//! [`GalleryAccessCode`] validates the 6 ASCII digits (`000000`–`999999`) stored in PostgreSQL
//! and compares submitted codes in constant time. [`CookieSigner`] holds a random 256-bit
//! in-memory HMAC-SHA256 key generated when `pp-api` starts, authenticating up to
//! [`MAX_UNLOCKED_GALLERIES`] `(gallery_id, share_link, code)` entries in the `HttpOnly`
//! [`UNLOCK_COOKIE_NAME`] cookie without storing client unlock sessions in the database.
//!
//! Each entry identifies its share link through a 32-byte keyed HMAC fingerprint rather than the
//! link's own token or any contact identifier, so two links to the same gallery (for example a
//! general link and a personal link) carry independent authorizations and neither link's bearer
//! token nor a contact id ever appears in the cookie. A leading record-format version byte keeps
//! legacy (gallery-scoped, pre-link-binding) cookie payloads from ever being reinterpreted as
//! valid link-scoped records.
//!
//! None of the types in this module prints raw key material, digits, or cookie tokens in
//! [`fmt::Debug`].

use std::{fmt, str::FromStr};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, KeyInit as _, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

use super::token::Token;

/// Length of a gallery access code in ASCII digits.
pub const ACCESS_CODE_DIGITS: usize = 6;

/// Name of the `HttpOnly` cookie recording unlocked share links in a client browser.
pub const UNLOCK_COOKIE_NAME: &str = "pp_gallery_unlock";

/// Maximum `(gallery_id, share_link, code)` entries retained in [`UNLOCK_COOKIE_NAME`].
pub const MAX_UNLOCKED_GALLERIES: usize = 16;

/// Byte width of a gallery's private share link token, matching `ShareToken` in `pp-database`.
pub const SHARE_LINK_ID_BYTES: usize = 16;

const KEY_BYTES: usize = 32;
const FINGERPRINT_BYTES: usize = 32;
const TAG_BYTES: usize = 32;
/// Record format version. Bumping it immediately invalidates every previously issued cookie,
/// including the legacy gallery-scoped (pre-link-binding) 48-byte-record format.
const RECORD_VERSION: u8 = 1;
const UNLOCK_RECORD_BYTES: usize = FINGERPRINT_BYTES + TAG_BYTES;
const MAX_COOKIE_RAW_BYTES: usize = 1 + MAX_UNLOCKED_GALLERIES * UNLOCK_RECORD_BYTES;
const MAX_COOKIE_ENCODED_LEN: usize = (MAX_COOKIE_RAW_BYTES * 8).div_ceil(6);

type HmacSha256 = Hmac<Sha256>;
/// A gallery's private share link token, identified only by its fixed byte width: callers pass
/// `ShareToken` (`pp-database`) or any other canonical token of the same width.
type ShareLinkId = Token<SHARE_LINK_ID_BYTES>;

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

/// Fixed-width `(link_fingerprint, hmac_tag)` entry inside [`UNLOCK_COOKIE_NAME`], naming its
/// share link only through a keyed HMAC fingerprint rather than the link's own token.
#[derive(Clone, Copy)]
struct UnlockRecord {
    fingerprint: [u8; FINGERPRINT_BYTES],
    tag: [u8; TAG_BYTES],
}

impl UnlockRecord {
    fn from_bytes(record: &[u8; UNLOCK_RECORD_BYTES]) -> Self {
        let mut fingerprint = [0u8; FINGERPRINT_BYTES];
        let mut tag = [0u8; TAG_BYTES];

        fingerprint.copy_from_slice(&record[..FINGERPRINT_BYTES]);
        tag.copy_from_slice(&record[FINGERPRINT_BYTES..]);

        Self { fingerprint, tag }
    }

    fn append_to(&self, buffer: &mut Vec<u8>) {
        buffer.extend_from_slice(&self.fingerprint);
        buffer.extend_from_slice(&self.tag);
    }

    fn matches_fingerprint(&self, fingerprint: &[u8; FINGERPRINT_BYTES]) -> bool {
        bool::from(self.fingerprint.ct_eq(fingerprint))
    }
}

/// In-memory HMAC-SHA256 signer and verifier for [`UNLOCK_COOKIE_NAME`].
///
/// [`Debug`](fmt::Debug) redacts the signing key so state dumps never leak secret bytes.
///
/// ```
/// use pp_core::types::access_code::{CookieSigner, GalleryAccessCode};
/// use pp_core::types::token::Token;
/// use uuid::Uuid;
///
/// let signer = CookieSigner::generate();
/// let gallery_id = Uuid::new_v4();
/// let link_id = Token::<16>::generate();
/// let code: GalleryAccessCode = "042819".parse().unwrap();
/// let cookie = signer.unlock(None, gallery_id, &link_id, &code);
///
/// assert!(signer.is_unlocked(Some(&cookie), gallery_id, &link_id, &code));
/// assert_eq!(format!("{signer:?}"), "CookieSigner(..)");
/// ```
#[derive(Clone)]
pub struct CookieSigner {
    key: [u8; KEY_BYTES],
}

impl CookieSigner {
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
    /// `(gallery_id, link_id, code)`.
    pub fn is_unlocked(
        &self,
        cookie_value: Option<&str>,
        gallery_id: impl Into<Uuid>,
        link_id: &ShareLinkId,
        code: &GalleryAccessCode,
    ) -> bool {
        let Some(cookie_value) = cookie_value else {
            return false;
        };
        let Some(records) = Self::decode_records(cookie_value) else {
            return false;
        };
        let fingerprint = self.link_fingerprint(gallery_id.into(), link_id);

        records.iter().any(|record| {
            record.matches_fingerprint(&fingerprint)
                && self.verify_tag(&fingerprint, code, &record.tag)
        })
    }

    /// Produces an updated base64url cookie value recording `(gallery_id, link_id, code)` while
    /// preserving up to `MAX_UNLOCKED_GALLERIES - 1` other previously unlocked share links.
    ///
    /// Two different links to the same gallery (for example a general link and a personal link)
    /// authorize independently: unlocking one never authorizes the other.
    pub fn unlock(
        &self,
        existing_cookie_value: Option<&str>,
        gallery_id: impl Into<Uuid>,
        link_id: &ShareLinkId,
        code: &GalleryAccessCode,
    ) -> String {
        let fingerprint = self.link_fingerprint(gallery_id.into(), link_id);
        let mut records = existing_cookie_value
            .and_then(Self::decode_records)
            .unwrap_or_default();

        records.retain(|record| !record.matches_fingerprint(&fingerprint));
        records.push(UnlockRecord {
            fingerprint,
            tag: self.sign_tag(&fingerprint, code),
        });

        if records.len() > MAX_UNLOCKED_GALLERIES {
            let overflow = records.len() - MAX_UNLOCKED_GALLERIES;

            records.drain(..overflow);
        }

        let mut payload = Vec::with_capacity(1 + records.len() * UNLOCK_RECORD_BYTES);

        payload.push(RECORD_VERSION);

        for record in &records {
            record.append_to(&mut payload);
        }

        URL_SAFE_NO_PAD.encode(payload)
    }

    /// Keyed HMAC naming `(gallery_id, link_id)` without exposing either in the cookie: domain
    /// separation (the `b"link"` tag prefix) keeps this derivation independent from
    /// [`Self::sign_tag`], which reuses the same signing key.
    fn link_fingerprint(&self, gallery_id: Uuid, link_id: &ShareLinkId) -> [u8; FINGERPRINT_BYTES] {
        // RFC 2104 HMAC-SHA256 accepts keys of every byte length; 32-byte keys cannot fail.
        #[allow(clippy::expect_used)]
        let mut mac =
            HmacSha256::new_from_slice(&self.key).expect("32-byte HMAC-SHA256 key is always valid");

        mac.update(b"link");
        mac.update(gallery_id.as_bytes());
        mac.update(link_id.as_str().as_bytes());

        mac.finalize().into_bytes().into()
    }

    fn mac(&self, fingerprint: &[u8; FINGERPRINT_BYTES], code: &GalleryAccessCode) -> HmacSha256 {
        // RFC 2104 HMAC-SHA256 accepts keys of every byte length; 32-byte keys cannot fail.
        #[allow(clippy::expect_used)]
        let mut mac =
            HmacSha256::new_from_slice(&self.key).expect("32-byte HMAC-SHA256 key is always valid");

        mac.update(b"tag");
        mac.update(fingerprint);
        mac.update(code.as_bytes());

        mac
    }

    fn sign_tag(
        &self,
        fingerprint: &[u8; FINGERPRINT_BYTES],
        code: &GalleryAccessCode,
    ) -> [u8; TAG_BYTES] {
        self.mac(fingerprint, code).finalize().into_bytes().into()
    }

    fn verify_tag(
        &self,
        fingerprint: &[u8; FINGERPRINT_BYTES],
        code: &GalleryAccessCode,
        tag: &[u8; TAG_BYTES],
    ) -> bool {
        self.mac(fingerprint, code).verify_slice(tag).is_ok()
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

        let (&version, body) = raw.split_first()?;

        if version != RECORD_VERSION || body.is_empty() {
            return None;
        }

        let (records, remainder) = body.as_chunks::<UNLOCK_RECORD_BYTES>();

        if !remainder.is_empty() {
            return None;
        }

        Some(records.iter().map(UnlockRecord::from_bytes).collect())
    }
}

impl fmt::Debug for CookieSigner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CookieSigner(..)")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
    fn debug_redacts_digits() {
        let code: GalleryAccessCode = "042819".parse().unwrap();

        assert_eq!(format!("{code:?}"), "GalleryAccessCode(..)");
        assert!(!format!("{code:?}").contains("042819"));
    }

    #[test]
    fn cookie_signer_binds_gallery_link_and_code_and_rejects_cross_link_reuse() {
        let signer = CookieSigner::from_bytes([0x33; KEY_BYTES]);
        let other_signer = CookieSigner::from_bytes([0x44; KEY_BYTES]);
        let generated = CookieSigner::generate();
        let code: GalleryAccessCode = "901234".parse().unwrap();
        let rotated_code: GalleryAccessCode = "901235".parse().unwrap();

        assert_eq!(format!("{signer:?}"), "CookieSigner(..)");
        assert_eq!(format!("{generated:?}"), "CookieSigner(..)");

        let gallery_a = Uuid::new_v4();
        let gallery_b = Uuid::new_v4();
        let link_a = ShareLinkId::generate();
        let link_b = ShareLinkId::generate();
        let cookie_a = signer.unlock(None, gallery_a, &link_a, &code);

        assert!(signer.is_unlocked(Some(&cookie_a), gallery_a, &link_a, &code));
        assert!(
            !signer.is_unlocked(Some(&cookie_a), gallery_b, &link_a, &code),
            "a link fingerprint is bound to its own gallery, not any gallery"
        );
        assert!(
            !signer.is_unlocked(Some(&cookie_a), gallery_a, &link_b, &code),
            "a different share link on the same gallery must not inherit the authorization"
        );
        assert!(!signer.is_unlocked(Some(&cookie_a), gallery_a, &link_a, &rotated_code));
        assert!(!other_signer.is_unlocked(Some(&cookie_a), gallery_a, &link_a, &code));
    }

    #[test]
    fn cookie_signer_keeps_multiple_links_of_one_gallery_independent_and_reuses_the_same_link() {
        let signer = CookieSigner::from_bytes([0x33; KEY_BYTES]);
        let code: GalleryAccessCode = "901234".parse().unwrap();
        let other_code: GalleryAccessCode = "555555".parse().unwrap();
        let gallery_id = Uuid::new_v4();
        let general_link = ShareLinkId::generate();
        let personal_link = ShareLinkId::generate();

        let cookie = signer.unlock(None, gallery_id, &general_link, &code);
        let cookie = signer.unlock(Some(&cookie), gallery_id, &personal_link, &other_code);

        assert!(signer.is_unlocked(Some(&cookie), gallery_id, &general_link, &code));
        assert!(signer.is_unlocked(Some(&cookie), gallery_id, &personal_link, &other_code));
        assert!(!signer.is_unlocked(Some(&cookie), gallery_id, &general_link, &other_code));

        // Reusing the same link rewrites its own entry in place rather than adding a duplicate.
        let reused = signer.unlock(Some(&cookie), gallery_id, &general_link, &code);
        let raw = URL_SAFE_NO_PAD.decode(&reused).unwrap();

        assert_eq!((raw.len() - 1) / UNLOCK_RECORD_BYTES, 2);
        assert!(signer.is_unlocked(Some(&reused), gallery_id, &general_link, &code));
        assert!(signer.is_unlocked(Some(&reused), gallery_id, &personal_link, &other_code));
    }

    #[test]
    fn cookie_signer_caps_at_sixteen_links_and_evicts_the_oldest_first() {
        let signer = CookieSigner::from_bytes([0x33; KEY_BYTES]);
        let other_signer = CookieSigner::from_bytes([0x44; KEY_BYTES]);
        let code: GalleryAccessCode = "901234".parse().unwrap();
        let mut links = Vec::new();
        let mut cookie: Option<String> = None;

        for _ in 0..18 {
            let gallery_id = Uuid::new_v4();
            let link_id = ShareLinkId::generate();
            let next = signer.unlock(cookie.as_deref(), gallery_id, &link_id, &code);

            assert!(!next.contains("901234"));
            assert!(signer.is_unlocked(Some(&next), gallery_id, &link_id, &code));
            cookie = Some(next);
            links.push((gallery_id, link_id));
        }

        let cookie = cookie.unwrap();

        for (gallery_id, link_id) in &links[0..2] {
            assert!(
                !signer.is_unlocked(Some(&cookie), *gallery_id, link_id, &code),
                "the two oldest links must be evicted once capacity is exceeded"
            );
        }

        for (gallery_id, link_id) in &links[2..18] {
            assert!(signer.is_unlocked(Some(&cookie), *gallery_id, link_id, &code));
            assert!(!other_signer.is_unlocked(Some(&cookie), *gallery_id, link_id, &code));
        }

        let (refreshed_gallery, refreshed_link) = links[2].clone();
        let refreshed = signer.unlock(Some(&cookie), refreshed_gallery, &refreshed_link, &code);
        let extra_gallery = Uuid::new_v4();
        let extra_link = ShareLinkId::generate();
        let after_extra = signer.unlock(Some(&refreshed), extra_gallery, &extra_link, &code);

        assert!(signer.is_unlocked(
            Some(&after_extra),
            refreshed_gallery,
            &refreshed_link,
            &code
        ));
        assert!(!signer.is_unlocked(Some(&after_extra), links[3].0, &links[3].1, &code));
        assert!(signer.is_unlocked(Some(&after_extra), extra_gallery, &extra_link, &code));
    }

    #[test]
    fn cookie_signer_rejects_tampering_malformed_and_legacy_gallery_scoped_records() {
        let signer = CookieSigner::from_bytes([0x33; KEY_BYTES]);
        let code: GalleryAccessCode = "901234".parse().unwrap();
        let gallery_id = Uuid::new_v4();
        let link_id = ShareLinkId::generate();
        let cookie = signer.unlock(None, gallery_id, &link_id, &code);

        let mut raw = URL_SAFE_NO_PAD.decode(&cookie).unwrap();
        let last = raw.len() - 1;
        raw[last] ^= 0x01;
        let forged = URL_SAFE_NO_PAD.encode(&raw);

        assert!(!signer.is_unlocked(Some(&forged), gallery_id, &link_id, &code));

        let padded = format!("{cookie}=");
        let non_canonical = format!("{}B", "A".repeat(63));
        // One byte short of a single new-format record body (after the version byte).
        let wrong_len = URL_SAFE_NO_PAD.encode({
            let mut bytes = vec![RECORD_VERSION];
            bytes.extend(std::iter::repeat_n(0x11u8, UNLOCK_RECORD_BYTES - 1));
            bytes
        });
        let oversized = URL_SAFE_NO_PAD.encode({
            let mut bytes = vec![RECORD_VERSION];
            bytes.extend(std::iter::repeat_n(
                0x11u8,
                (MAX_UNLOCKED_GALLERIES + 1) * UNLOCK_RECORD_BYTES,
            ));
            bytes
        });

        // Legacy (pre-link-binding) 48-byte-per-record payloads, with and without a version byte
        // that happens to collide with the current one, can never parse as valid new records:
        // `48 * n` is never congruent to `1 (mod UNLOCK_RECORD_BYTES)` for the new 64-byte width.
        let legacy_single = URL_SAFE_NO_PAD.encode(vec![0x11u8; 48]);
        let legacy_versioned = URL_SAFE_NO_PAD.encode({
            let mut bytes = vec![RECORD_VERSION];
            bytes.extend(vec![0x11u8; 48 - 1]);
            bytes
        });
        let legacy_multi_versioned = URL_SAFE_NO_PAD.encode({
            let mut bytes = vec![RECORD_VERSION];
            bytes.extend(vec![0x11u8; 48 * 4 - 1]);
            bytes
        });

        for invalid in [
            None,
            Some(""),
            Some(padded.as_str()),
            Some(non_canonical.as_str()),
            Some(wrong_len.as_str()),
            Some(oversized.as_str()),
            Some(legacy_single.as_str()),
            Some(legacy_versioned.as_str()),
            Some(legacy_multi_versioned.as_str()),
        ] {
            assert!(
                !signer.is_unlocked(invalid, gallery_id, &link_id, &code),
                "{invalid:?}"
            );
        }

        let recovered = signer.unlock(Some(&wrong_len), gallery_id, &link_id, &code);

        assert!(signer.is_unlocked(Some(&recovered), gallery_id, &link_id, &code));
    }

    #[test]
    fn cookie_bytes_never_contain_the_access_code_the_link_token_or_the_gallery_id() {
        let signer = CookieSigner::from_bytes([0x33; KEY_BYTES]);
        let code: GalleryAccessCode = "901234".parse().unwrap();
        let gallery_id = Uuid::new_v4();
        let link_id = ShareLinkId::generate();
        let cookie = signer.unlock(None, gallery_id, &link_id, &code);
        let raw = URL_SAFE_NO_PAD.decode(&cookie).unwrap();

        assert!(!cookie.contains(code.as_str()));
        assert!(!cookie.contains(link_id.as_str()));

        let gallery_id_bytes = gallery_id.as_bytes();

        assert!(
            !raw.windows(gallery_id_bytes.len())
                .any(|window| window == gallery_id_bytes),
            "the gallery id must only ever appear as a keyed HMAC fingerprint, never in the clear"
        );
    }
}
