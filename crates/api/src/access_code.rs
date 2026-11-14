//! Re-exports shared per-gallery 6-digit access-code encryption and unlock-cookie primitives
//! from [`pp_core::types::access_code`], verified against [`GalleryId`] in API unit tests.

pub use pp_core::types::access_code::{
    ACCESS_CODE_DIGITS, AccessCodeCipher, AccessCodeCryptoError, ENCRYPTED_ACCESS_CODE_LEN,
    EncryptedAccessCode, EncryptionKey, EncryptionKeyValueParser, GalleryAccessCode, KEY_BYTES,
    MAX_UNLOCKED_GALLERIES, ParseAccessCodeError, ParseEncryptionKeyError, UNLOCK_COOKIE_NAME,
};

#[cfg(test)]
mod tests {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    use pp_core::types::generate::Generate;
    use pp_database::gallery::GalleryId;

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
