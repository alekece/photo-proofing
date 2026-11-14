# Gallery sharing studio view with live email preview and invitations

Issue: [#348](https://github.com/alekece/photo-proofing/issues/348). Builds on `GalleryStudio` ([#342](https://github.com/alekece/photo-proofing/issues/342)), workspace & gallery contacts ([#339](https://github.com/alekece/photo-proofing/issues/339)), and 6-digit gallery access codes ([#344](https://github.com/alekece/photo-proofing/issues/344)). Prepares shared SMTP transport and Warm Paper email rendering for automatic expiration reminders ([#349](https://github.com/alekece/photo-proofing/issues/349)) and sent-email activity events ([#350](https://github.com/alekece/photo-proofing/issues/350)).

Touches `Cargo.toml`, `deny.toml` (only if required by permissive TLS crates), `config.example.toml`, `crates/core`, `crates/database`, `crates/api`, `crates/image-worker`, `web/`, `docs/SPECIFICATIONS.md`, `docs/releases/MVP.md`, and `docs/designs/ui/screens/gallery.md`. Requires no SQL schema migration (`jobs.payload` is already tagged JSONB).

## Intent

Sharing a gallery should feel like a complete, editorial client communication workflow integrated naturally into the gallery studio rather than a bare copy-link dialog. Photographers compose and preview a Warm Paper invitation card inline inside `Studio → Share`, automatically link new `To` recipients to the workspace and gallery contact directory, transition a `Draft` gallery to `Prepublish` on send, and deliver multipart HTML + plain-text emails through a durable background SMTP job — while keeping direct link activation available in `Settings → Privacy & Protection` when SMTP is unconfigured or not needed.

## Goals

- **Integrated `Studio → Share` view (`GalleryStudio` & `GalleryActionBar`):**
  - Add `Share` (`Send` icon) to the left rail `Studio` group (`Design`, `Share`, `Contacts`, `Settings`), disabled when the gallery is `archived` (with tooltip explaining archived galleries cannot be shared).
  - Clicking the top-bar `Share` button (`GalleryActionBar`) opens `Studio → Share` instead of opening `PublishDialog(kind='share')` directly.
  - While `Studio → Share` is active, the top Journey Wizard (`Upload → Share → Collect → Publish`) stays visible and the top-right actions in `GalleryActionBar` swap cleanly to **`Cancel`** (returning to the previously active studio section, defaulting to `photos`) and **`✉ Send invitation (N)`** (where `N = to.length + bcc.length`, disabled when `N === 0`, the gallery is `expired`, SMTP is unconfigured, or a send is in flight). `Copy link` is omitted from `Share` and no `Desktop | Phone` toggle is shown.
  - After a successful send, show a localized confirmation toast (`"Invitation queued"` / `"N invitations queued"`), refresh linked gallery contacts, and return to the previously active studio section (`photos` by default) *(design call)*.
- **Direct link activation without email (`Settings → Privacy & Protection`):**
  - Keep `PublishDialog(kind='share')` intact for direct-link-only sharing: when a gallery is still in `Draft` (`share_token === null`), the `Client link` row in `GallerySettingsPanel` offers an inline **`Share link`** button that opens `PublishDialog(kind='share')` (preserving `FR-04` empty/uploading/failed proof warnings) to create and reveal the `ShareLink` copy control without sending an email or requiring SMTP.
- **Two flush `660 px` Warm Paper sheets in `Studio → Share`:**
  1. **Envelope card (`max-w-[660px]`):**
     - **`To` row:** Pre-populated with contacts already linked to the gallery when first opened (if any, removable as chips before sending without unlinking them from the gallery), `+ Contacts` picker over workspace contacts, and inline email entry (`email@example.com` or `Name <email@example.com>` on `Enter`, `,`, or blur). Sending auto-creates any new `To` address in Workspace Contacts (deriving the display name from the `Name` prefix or the local part before `@` when absent) and links every `To` contact to the gallery (`gallery_contacts`).
     - **Optional `+ BCC` toggle & row:** Private `Email` chips (`Enter`, `,`, or blur); never saved to Workspace Contacts and never linked to `gallery_contacts`.
     - **Editable `Subject` input:** Pre-filled with localized stage-appropriate default (`Prepublish`/Draft proofing vs `Publish` finished-photo delivery), resetting to the new stage default only while untouched by the photographer.
  2. **Live Email Preview card (`max-w-[660px]`, visually joined flush below Envelope):**
     - Fixed **Studio Signature** (`StudioSignature` using workspace `studio_name`, `signature`, `signature_size`, and logo from `/api/v1/settings`).
     - Fixed **Cover Photograph** (`3:2` clean hero preview honoring `focal_point`, or `CoverBackdrop` when `hero_id === null`).
     - **Gallery title & metadata line** (formatted `shoot_date` when set, plus ready photo count — proof count before delivery, final count once delivered).
     - **Auto-sizing inline message `<textarea>`** (no scrollbar, no resize handle; pre-filled with a localized stage-appropriate greeting/message, editable directly inside the Warm Paper preview).
     - Fixed **Access Code block** when the gallery has a 6-digit access code configured (`lockedAccessCode` / `expectedAccessCode`), displaying the 6-digit monospace code clearly.
     - Fixed **`View Gallery` CTA** (styled preview button in the composer; real link in sent emails).
- **Stage transitions & proof readiness confirmation:**
  - Sending from a `Draft` gallery first flushes pending gallery edits (including any pre-share `access_code` save, refusing if `expectedAccessCode === 'invalid'`). If the `Draft` gallery has zero ready proofs or proofs still uploading/failed, confirm via `ConfirmDialog` (matching `FR-04`'s empty/unfinished share warning) before sending.
  - The server-side invitation endpoint atomically transitions a `Draft` gallery to shared (`Prepublish`, generating `galleries.share_token` if absent and locking the access code), upserts/links `To` contacts, and enqueues outbound email jobs.
  - When the gallery is in `Publish` (`delivered`), default subject, default message body, and photo count reflect finished-photo delivery.
- **Optional `[smtp]` configuration & durable worker delivery (`crates/`):**
  - Optional `[smtp]` section shared by `pp-api` and `pp-image-worker` (`host`, `port`, `tls` mode `tls | starttls | none` for local LAN / Mailpit trials, optional `username` and redacted `password` via `PP_SMTP_PASSWORD`, `from_email`, and optional `from_name`), documented with synthetic placeholders in `config.example.toml`.
  - Expose `email_enabled: bool` on `GET /api/v1/settings` (`SettingsView`). When `email_enabled` is `false`, `Studio → Share` shows a clear informational banner explaining SMTP is not configured on this installation and pointing to `Settings → Privacy & Protection` for the direct client link, while disabling `Send invitation`.
  - Share `[gallery_password]` (`AccessCodeCipher` / `EncryptionKey`) and `app_origin` (`Origin`) across `pp-core`, `pp-api`, and `pp-image-worker` so `JobPayload::SendEmail` stores **only** identifiers and text (`workspace_id`, `gallery_id`, recipient enum `Contact(ContactId)` vs `Bcc(Email)`, `stage`, `subject`, `message`) — never plaintext `ShareToken`s or 6-digit access codes in `jobs.payload`.
  - `pp-image-worker` resolves the gallery, workspace settings, studio account `Reply-To` email, cover thumbnail + logo files from `PhotoStorage`, recipient URL (`/g/{per_contact_token}` for `To`, `/g/{gallery_share_token}` for `BCC`), and decrypted 6-digit access code at send time, building an individual multipart (`text/plain` + `text/html` with inline `cid:` cover JPEG and PNG logo when present) email via `lettre` and sending one message per recipient (`BCC` recipients never see other recipients).

## Non-goals

- Automatic 3-day expiration reminder sweeps and per-gallery reminder toggle ([#349](https://github.com/alekece/photo-proofing/issues/349), which will reuse this crate's SMTP mailer and Warm Paper email template).
- Chronological `activity_events` persistence and `/activity` UI ([#350](https://github.com/alekece/photo-proofing/issues/350)).
- Tracking email opens/pixels, bounce webhooks, or rich-HTML WYSIWYG formatting in the message body (plain text with preserved paragraph/line breaks rendered cleanly into the Warm Paper template).
- Sending SMS messages.

## Anti-goals

- Storing raw `ShareToken`s, decrypted 6-digit `GalleryAccessCode`s, or SMTP credentials in `jobs.payload`, logs, `Debug` output, or tracing spans.
- Exposing other recipients' addresses in `To` or `Cc` headers when multiple contacts or `BCC` addresses are invited at once (every recipient receives their own individual SMTP message).
- Breaking local-network or offline-SMTP deployments when `[smtp]` is omitted from configuration.

## Constraints

- **Product specification update (`AGENTS.md`):** `docs/SPECIFICATIONS.md` (`FR-04` and `Scope & Exclusions`) and `docs/releases/MVP.md` currently mark email sending as excluded from MVP; this design explicitly promotes optional built-in SMTP gallery invitation emails into MVP scope while keeping SMS out of scope.
- **Dependencies (`Cargo.toml` & `deny.toml`):** Add `lettre` in `[workspace.dependencies]` with minimal features (`builder`, `smtp-transport`, `tokio1-rustls-tls` or compatible async transport) verified against `cargo deny check` (advisories, licenses, bans).
- **Rust rules (`crates/AGENTS.md` & `REVIEW.md`):**
  - Zero free functions outside `#[cfg(test)]`.
  - Move `AccessCodeCipher` / `EncryptionKey` from `crates/api/src/access_code.rs` into `pp-core` (keeping client unlock-cookie encoding in `pp-api` or `pp-core` cleanly) so both `pp-api` and `pp-image-worker` decrypt `access_code_ciphertext` without duplicating crypto code.
  - Put shared `SmtpConfig` (with redacted `Debug` on `SmtpPassword`) in `pp-core::config` (flattened into `ApiConfig` and `WorkerConfig`; `ApiConfig` only needs to know whether `[smtp]` is configured + validate config consistently, while `WorkerConfig` constructs the `Mailer` and also reads `[auth] app_origin` and `[gallery_password] encryption_key`).
  - If a gallery has `access_code_ciphertext IS NOT NULL` and the worker lacks `PP_GALLERY_PASSWORD_KEY`, or if the worker lacks `[smtp]`, `SendEmail` fails explicitly so retries/operators see the misconfiguration rather than silently omitting the required access code or dropping the email.
  - If the gallery was deleted, archived, or expired (`expires_at <= now()`), or if a `To` contact was unlinked/deleted before the worker runs the job, complete the job cleanly as a no-op so dead links are never emailed *(design call)*.
- **Frontend (`web/AGENTS.md`):**
  - PascalCase Svelte components (`GallerySharePanel.svelte`), all strings in `web/messages/{en,fr}.json` (both UI controls and default EN/FR email subject/body + worker email labels following workspace `Language`), zero `vitest-axe` violations, and clean responsive layout down to `390 px`.

## Approaches

- **A. Per-recipient `JobPayload::SendEmail` resolved in `pp-image-worker` + inline `cid:` cover/logo parts (chosen).**
  - `POST /api/v1/galleries/{id}/invitations` runs one DB transaction that shares the gallery if still in `Draft`, upserts/links `To` contacts, and enqueues one `SendEmail` job per recipient carrying only IDs and text.
  - `pp-image-worker` loads current state, decrypts any access code in memory, attaches existing `PhotoStorage` clean cover thumbnail JPEG (`Variant::Thumbnail`) and `WorkspaceSettings.logo` PNG as `cid:` inline parts (`multipart/alternative` + `multipart/related`), and sends via `lettre`.
  - *Trade-off:* Moves `AccessCodeCipher` + `app_origin` + `[smtp]` into shared config read by the worker, in exchange for zero secret leakage in `jobs.payload`, independent per-recipient retries without extra tracking tables, reliable images on private LANs and webmail, and turnkey reuse in #349 and #350.
- **B. Single batch job per send + remote `<img src>` URLs.**
  - Fewer rows in `jobs`, but partial SMTP failures either duplicate emails to earlier recipients on retry or need a separate batch-recipient state table, and remote `<img src>` URLs fail on private LANs and webmail image proxies; rejected.
- **C. Pre-render MIME/HTML in `pp-api` and persist in `jobs.payload`.**
  - Avoids sharing `AccessCodeCipher` with the worker, but writes decrypted 6-digit access codes and bearer `ShareToken` links into PostgreSQL JSON and cannot generate worker-driven reminders in #349; rejected.

## Decisions

- **API contract (`POST /api/v1/galleries/{id}/invitations`):**
  - Request body:
    - `to: Vec<InvitationContactInput>` (`{ email: Email, name: Option<ContactName> }`)
    - `bcc: Vec<Email>` (`#[serde(default)]`)
    - `subject: EmailSubject` (trimmed, `1..=200` chars, no CR/LF/NUL)
    - `message: EmailMessage` (trimmed, `1..=5000` chars, no NUL)
  - Rejects with:
    - `503 Service Unavailable` (`email_unavailable`) when `[smtp]` is not configured;
    - `409 Conflict` (`gallery_archived` or `gallery_expired`) when the gallery is archived or expired (`expires_at <= now()`); in `Studio → Share`, an expired gallery shows an inline notice pointing to `Settings → General → Open until` and disables `Send invitation` *(design call)*;
    - `400 Bad Request` when `to.is_empty() && bcc.is_empty()`, when total recipients (`to.len() + bcc.len()`) exceed an operator-configurable cap (`[smtp] max_recipients_per_send`, default `50`), or when subject/message fail validation.
  - Deduplicates recipients case-insensitively (`to` wins if the same email is listed in both `to` and `bcc`).
  - For each `to` entry: finds existing workspace contact by normalized `Email` or creates one (using supplied `name` or fallback derived from the email's local part), then idempotently links `(gallery_id, contact_id)` in `gallery_contacts`.
  - Ensures the gallery is shared (`shared_at = COALESCE(shared_at, now())`, generating `share_token` if `None`), enqueues one `JobPayload::SendEmail` per unique recipient in the same transaction, and returns `200 OK` with `{ gallery: GalleryView, queued_count: usize }` so the studio immediately updates `share_token`, `states`, and contact counts without a separate round-trip.
- **Email headers & localization:**
  - `From`: `"{studio_name}" <{smtp.from_email}>` when `studio_name` (or `smtp.from_name`) is non-empty, otherwise `<{smtp.from_email}>`.
  - `Reply-To`: the workspace photographer account's email (`accounts.email`).
  - Fixed templatechrome (`Access code`, `Open until …`, `View Gallery`, plain-text footer) follows `WorkspaceSettings.language` (`En` / `Fr`).
- **Documentation updates:** Update `docs/SPECIFICATIONS.md` (`FR-04`, `Scope & Exclusions`), `docs/releases/MVP.md`, `config.example.toml`, and `docs/designs/ui/screens/gallery.md`.

## Left to the builder

- Exact `lettre` transport/TLS feature flags that satisfy `cargo deny check` cleanly while supporting `tls`, `starttls`, and `none` (local Mailpit/SMTP test server).
- Exact HTML table/inline-CSS structure of the Warm Paper email so it matches the `660 px` live studio preview across desktop/mobile mail clients while degrading gracefully when `hero_id` or `logo` is absent.
- Deterministic SMTP integration testing in Rust (e.g., local loopback TCP mock SMTP server asserting MIME headers, `Reply-To`, `cid:` parts, per-contact vs generic `BCC` URLs, access-code block, and redaction) and component + `vitest-axe` tests in `web/`.
