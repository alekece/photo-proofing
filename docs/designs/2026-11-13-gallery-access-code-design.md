# Per-gallery 6-digit access code and dashboard card access icons

Issue: [#344](https://github.com/alekece/photo-proofing/issues/344). Builds on `GallerySettingsPanel` (#342) and touches `crates/database`, `crates/api`, `config.example.toml`, `web/`, `docs/SPECIFICATIONS.md`, and `docs/releases/MVP.md`.

## Intent

Photographers need optional access-code protection for sensitive client galleries — using a simple 6-digit numeric code (`000000`–`999999`) with one-click generation and copying — plus subtle inline icons on dashboard cards showing at a glance which galleries require a code or are closed to clients.

## Goals

- **Per-gallery 6-digit access code (`Studio → Settings → Privacy & Protection`):**
  - Configured per gallery (alongside `Client link` and `Enable watermarks`), never as a global workspace password.
  - **In `Draft` (before sharing, `share_token IS NULL`):** `Require access code` switch + 6-digit numeric input (`inputmode="numeric"`, `000000`–`999999`; enabling the switch auto-generates a random 6-digit code if empty) + **`↻ Generate`** button + **`Copy`** button.
  - **Once shared (`share_token IS NOT NULL`):** Configuration locks (the switch/generate/edit controls become read-only; if a code is set, display the 6-digit monospace code + one-click **`Copy`**; if shared without a code, show that none was set and cannot be added after sharing). Enforced server-side (`409 Conflict` if a `PATCH` tries to change a shared gallery's access code).
- **Encrypted at rest via optional operator key:**
  - Stored encrypted in PostgreSQL (`ChaCha20-Poly1305` AEAD with random 96-bit nonce + gallery-ID AAD) using an optional operator-configured 256-bit secret (`[gallery_password] encryption_key` / `PP_GALLERY_PASSWORD_KEY`).
  - Existing deployments without `PP_GALLERY_PASSWORD_KEY` boot normally; enabling an access code when no key is configured returns a clear error (`503` / localized message).
  - Never log or print the key, plaintext 6-digit code, ciphertext, or unlock cookie in `Debug` or tracing spans (`crates/AGENTS.md`).
  - Decryptable by the backend so the photographer's `GET /api/v1/galleries/{id}` view (and future #348/#349 invitation/reminder emails) can show the 6-digit code, while `GET /api/v1/galleries` (`GalleryOverview`) only exposes `access_code_protected: boolean` (or `password_protected: boolean` — omitting the code from dashboard list responses).
- **Client access-code gate (`/g/{token}`):**
  - Active (`shared_at IS NOT NULL`, non-expired, non-archived) code-protected galleries show a full-screen gate with the **Studio Signature** at top and **Gallery Title** (+ optional formatted `shoot_date`) centered over a **darkened clean cover photograph** (`brightness ~0.38` + vignette, or warm dark `CoverBackdrop` if no cover is set), with a frosted 6-digit numeric input (`inputmode="numeric"`, `autocomplete="one-time-code"`) + **`Enter`** button.
  - Before unlock, ONLY the minimal gate metadata (`locked: true`, `title`, `shoot_date`, `has_cover`, studio signature/logo) and `/shared/{token}/cover` + `/shared/{token}/logo` are served; `/photos`, `/photos/{id}/thumbnail|preview|download`, `/download`, `/favorites`, `/comments`, and `/open` all return `401 Unauthorized` (`gallery_locked`) until unlocked!
  - Unshared (`Draft`), expired, archived, or unknown links STILL return `404` and render the generic unbranded unavailable page (never revealing whether a code existed).
  - Unlock endpoint (`POST /api/v1/galleries/shared/{token}/unlock`, wrapped with `from_fn(FailedAttempts::limit)` per-peer rate limiting and constant-time comparison) sets a stateless HMAC-SHA256-signed `HttpOnly`, `SameSite=Lax` cookie scoped to `/`, bound to `(gallery_id, ciphertext_sha256)` so both the general gallery link and personal contact links (#339) work seamlessly once unlocked.
  - Owner previews (`ClientPreview` and `Design` live preview) use authenticated photographer endpoints and stay unlocked without needing the code.
- **Dashboard card inline icons (`GalleryCard`):**
  - Subtle inline icons right beside the gallery `<h2>` title: **`Lock`** when protected by an access code, and **`EyeOff`** when client access is closed (`Expired` or `Archived`), with accessible labels/tooltips in EN and FR.

## Non-goals

- Post-share code rotation or per-contact separate codes (one optional 6-digit code per gallery, locked once shared).
- Changing photographer login authentication.

## Constraints

- **Dependencies (`Cargo.toml` & `deny.toml`):** Use RustCrypto crates with permissive `MIT OR Apache-2.0` licenses (`chacha20poly1305` and `hmac`, reusing existing `sha2`, `rand`, `base64`), verified with `cargo deny check`.
- **Database schema (`0033_gallery_access_code.sql` — numbered `0033` alongside #347's `0032`):**
  - `ALTER TABLE galleries ADD COLUMN access_code_ciphertext bytea CONSTRAINT galleries_access_code_ciphertext_len CHECK (access_code_ciphertext IS NULL OR octet_length(access_code_ciphertext) = 34);` (`12`-byte ChaCha20Poly1305 nonce + `6`-byte ASCII digits + `16`-byte Poly1305 tag = `34` bytes).
- **Rust rules (`crates/AGENTS.md`):**
  - Validated `GalleryAccessCode` (`^[0-9]{6}$`, omitting raw `Debug`), `EncryptedAccessCode` (`[u8; 34]`, deriving `sqlx::Type` if `Vec<u8>`/`[u8; 34]`, omitting raw `Debug`), and `AccessCodeCipher` (wrapping the 32-byte key with redacted `Debug`, binding `gallery_id.0.as_bytes()` as AEAD AAD so ciphertext cannot be transplanted across galleries!). Zero free functions outside `#[cfg(test)]`.

## Approaches

- **A. `ChaCha20-Poly1305` (with `gallery_id` AAD) at rest + stateless HMAC-SHA256 `HttpOnly` unlock cookie (chosen).** Constant-size 34-byte column, cryptographic gallery binding, zero database writes per client unlock, and works across general and per-contact links.
- **B. Server-side client unlock sessions table.** Adds DB writes/cleanup sweeps for anonymous viewers without benefit; rejected.

## Decisions

- Plaintext format: strictly 6 ASCII digits (`000000`–`999999`), validated by `GalleryAccessCode`.
- Config section in `crates/api/src/config.rs` & `config.example.toml`: optional `[gallery_password]` (`encryption_key` / `PP_GALLERY_PASSWORD_KEY`, 32 random bytes encoded as 43-char unpadded base64url or 64-char hex; `secure_cookie` follows `session.secure_cookie`).
- Unlock cookie: `pp_gallery_unlock` (holding a bounded list of up to 16 unlocked `gallery_id:tag` entries so unlocking gallery B does not evict gallery A), `HttpOnly`, `SameSite=Lax`, `Path=/`.
- Update `docs/SPECIFICATIONS.md` (`FR-03`, `FR-04`, `Scope & Exclusions`, `AC-01`), `docs/releases/MVP.md`, and screen docs (`dashboard.md`, `gallery.md`, `client.md`).

## Left to the builder

- Exact frosted-glass styling of the 6-digit client gate input over the darkened cover photo so it matches the Warm Editorial dark aesthetic on desktop and `390 px` mobile.
- Comprehensive crypto, rate-limit, access-boundary (`404` vs `401`), migration, and `vitest-axe` tests in EN and FR.
