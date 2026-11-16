# Plain 6-digit gallery access code and in-memory unlock cookie signing

Issue: [#375](https://github.com/alekece/photo-proofing/issues/375). Supersedes the at-rest encryption and `[gallery_password]` configuration in [#344](https://github.com/alekece/photo-proofing/issues/344) / [#358](https://github.com/alekece/photo-proofing/pull/358) and updates `docs/SPECIFICATIONS.md`, `docs/releases/MVP.md`, `docs/designs/2026-11-13-gallery-access-code-design.md`, and `docs/designs/2026-11-14-gallery-sharing-invitations-design.md`.

## Intent

Remove the unnecessary `PP_GALLERY_PASSWORD_KEY` operator secret and `ChaCha20-Poly1305` at-rest encryption for 6-digit gallery access codes, storing validated `GalleryAccessCode` values directly in PostgreSQL and signing client `pp_gallery_unlock` cookies with an in-memory 256-bit HMAC key generated when `pp-api` starts.

## Goals

- **Plain validated `GalleryAccessCode` in PostgreSQL:**
  - Replace `galleries.access_code_ciphertext bytea` with `galleries.access_code text CONSTRAINT galleries_access_code_six_digits CHECK (access_code IS NULL OR access_code ~ '^[0-9]{6}$')`.
  - Derive `sqlx::Type` (`#[sqlx(transparent)]`) on `GalleryAccessCode`, keeping redacted `fmt::Debug` and constant-time `verify`. Photographer `Gallery` / `GalleryOverview` responses serialize `access_code: Option<GalleryAccessCode>` directly (dropping `GalleryView`, `serialize_presence`, and `access_code_protected`), while client `SharedGallery` responses omit it.
  - Migrate any existing row where `access_code_ciphertext IS NOT NULL` to a random 6-digit `access_code` before dropping `access_code_ciphertext` so locked galleries never silently become public.
- **In-memory `CookieSigner` in `pp-api`:**
  - Generate a random 32-byte HMAC-SHA256 key in `ApiState::new` (`CookieSigner::generate()`), accessed via `state.cookie_signer()` (`.unlock(...)` / `.is_unlocked(...)`). On a rare `pp-api` restart or redeploy, clients re-enter the 6-digit code once; saved favorites and comments in PostgreSQL are unaffected.
  - `pp_gallery_unlock` (`HttpOnly`, `SameSite=Lax`, `Path=/`) stores up to 16 fixed-width 48-byte entries `gallery_id (16B) || HMAC-SHA256(key, gallery_id || access_code) (32B)` encoded as unpadded base64url. Neither the 6 digits nor an unsalted hash ever leaves the server, gallery IDs cannot be transplanted across entries, changing or clearing a code invalidates existing tags, and unthrottled photo/favorite/comment/download routes verify only the 256-bit HMAC tag.
- **Delete redundant crypto, configuration, and `503` failure modes:**
  - Remove `PP_GALLERY_PASSWORD_KEY`, `[gallery_password]` (`GalleryPasswordConfig`), `EncryptionKey`, `EncryptionKeyValueParser`, `EncryptedAccessCode`, `AccessCodeCipher`, the `chacha20poly1305` dependency, generic `<C>` on `GalleryChanges` (`try_map_access_code`), worker decryption in `send_email.rs`, `ErrorCode::AccessCodeUnavailable` (`503`) in `pp-api` and `web/` i18n, and `PP_GALLERY_PASSWORD_KEY` in `deploy/compose.yml`, `config.example.toml`, and `README.md`.

## Non-goals

- Changing the client `/g/{token}` 6-digit gate UX, Draft-only edit lock (`409 access_code_locked` once shared), or `FailedAttempts::limit` rate limiting on `POST /api/v1/galleries/shared/{token}/unlock`.

## Constraints

- Never log or print the 6-digit `GalleryAccessCode`, HMAC key, or `pp_gallery_unlock` cookie value in `Debug` or tracing spans (`crates/AGENTS.md`).
- Keep zero free functions in non-test Rust code (`crates/AGENTS.md`).

## Approaches

- **A. Singleton HMAC key table in PostgreSQL.** Preserves client unlock cookies across `pp-api` restarts, but adds schema and startup/repository code while placing the cookie-signing secret in database dumps alongside `share_token`; rejected.
- **B. Plain `GalleryAccessCode` column + in-memory 32-byte HMAC key on `pp-api` startup (chosen).** Matches `crates/AGENTS.md` (in-memory state for data cheap to lose), keeps the cookie-signing key strictly in RAM, rotates automatically on redeploy, and eliminates ~350 lines of key/cipher/error plumbing.
