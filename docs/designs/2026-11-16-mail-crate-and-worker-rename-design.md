# Pure `pp-mail` crate, `pp-worker` rename, and aligned Warm Paper invitation emails

Issue: [#374](https://github.com/alekece/photo-proofing/issues/374). Updates `docs/SPECIFICATIONS.md`, `docs/releases/MVP.md`, `docs/designs/2026-11-13-expiration-presets-design.md`, and `docs/designs/2026-11-14-gallery-sharing-invitations-design.md`.

## Intent

Separate email rendering and SMTP delivery from libvips/SQL worker concerns into a pure `pp-mail` crate using compile-time `askama` templates, rename `pp-image-worker` consistently to `pp-worker` now that it executes general background jobs, and eliminate the three user-visible discrepancies between sent Warm Paper invitation emails and the live studio preview—including simplifying `galleries.expires_at` to a plain calendar `DATE` (`YYYY-MM-DD`, valid through end of day UTC) so studio, client pages, sent emails, and future `#349` scheduled reminders always show the exact same calendar day.

## Goals

- **Consistent `pp-worker` (`crates/worker`) naming:**
  - Rename `crates/image-worker` to `crates/worker` (`package.name = "pp-worker"`, binary `pp-worker`).
  - Update workspace `Cargo.toml`, `crates/worker/tests/shutdown.rs`, `docker/worker.Dockerfile`, `deploy/compose.yml` (`ghcr.io/alekece/pp-worker:staging`), `.github/workflows/ci.yml` (`name: worker`), `.github/workflows/cd.yml` (`name: worker`, `ghcr.io/alekece/pp-worker`), `config.example.toml`, `README.md`, and active docs.
- **Dedicated pure `pp-mail` (`crates/mail`) crate with `askama`:**
  - Create `crates/mail` (`package.name = "pp-mail"`) with zero `libvips` and zero `sqlx` dependencies, depending only on `pp-core` (default features), `askama`, `lettre`, `thiserror`, and `time`.
  - Move small shared pure domain types (`Language`, `CoverSignature`, `FocalCoordinate`, `FocalCoordinateError`, `FocalPoint`, and `InvitationStage`) to `pp-core` (gating `sqlx::Type` / `sqlx::FromRow` derives behind `pp-core`'s existing `sqlx` feature) and re-export them from `pp-database` so existing database/API callers stay clean.
  - Replace raw `format!` HTML assembly and manual `HtmlEscaped` in `mail.rs` with compile-time `askama` templates (`crates/mail/templates/warm_paper.html` and `crates/mail/templates/warm_paper.txt`), preserving email-client-safe `<table role="presentation">` markup, inline CSS, CR/LF display-name header sanitization, and `multipart/alternative` + `multipart/related` `cid:` attachments (`studio-logo@photoproofing`, `gallery-cover@photoproofing`).
  - Structure `WarmPaperEmail` copy/kind cleanly (`EmailKind::Invitation(InvitationStage)` today) so `#349` automatic expiration reminders can add a variant without duplicating templates or MIME/SMTP assembly.
- **Align sent emails with `GallerySharePanel.svelte`:**
  - **Metadata line:** Add `shoot_date: Option<time::Date>` and `photo_count: usize` (ready proof count at the gallery's watermark setting for `Proofing`; ready final count for `Finals`, via `PhotoRepository::count_ready`) to `WarmPaperEmail`. Render `{shoot_date} · {photo_count} photo(s)` beneath the gallery title in both HTML and plain-text templates (localized in English and French using the same short month format and plural rules as `GallerySharePanel.svelte`, omitting `shoot_date` when `None`).
  - **Access-code placement:** Render the 6-digit access-code card **above** the primary CTA button in `warm_paper.html` (and above the CTA link line in `warm_paper.txt`).
- **Plain calendar `DATE` for `galleries.expires_at`:**
  - Add migration `0036_gallery_expires_at_date.sql` converting `galleries.expires_at` from `timestamptz` to `date` (`ALTER TABLE galleries ALTER COLUMN expires_at TYPE date USING (expires_at AT TIME ZONE 'UTC')::date`).
  - Define gallery expiration in SQL as `(expires_at IS NOT NULL AND expires_at < (now() AT TIME ZONE 'UTC')::date)` across `GalleryRepository` (`COLUMNS`, `GalleryFilter::Expired`, `list`, `find_shared`, `find_shared_gate`) and `ContactRepository` personal share-token checks—meaning a gallery with `expires_at = '2026-10-31'` remains accessible through `2026-10-31 23:59:59 UTC` and becomes expired starting `2026-11-01 00:00:00 UTC`.
  - Change `expires_at` from `Option<Timestamp>` to `Option<time::Date>` on `Gallery`, `SharedGallery`, `GalleryChanges` (`Option<Option<time::Date>>`), and `WarmPaperEmail`, serialized on the wire as `"YYYY-MM-DD"` (identical to `shoot_date`).
  - Simplify `web/`: update `expires_at` schema to `YYYY-MM-DD`, teach `formatShortDate` / `formatLongDate` / `formatFullDate` to parse bare `YYYY-MM-DD` calendar dates in UTC so no browser timezone shifts them, remove `endOfLocalDay` and `toLocalDate`, and remove the `onMount` UTC hydration workaround in `ExpiryBadge.svelte`.

## Non-goals

- Sharing a single template file between SvelteKit and Rust: `GallerySharePanel.svelte` is an interactive inline editor (`<textarea>` inside the card + `<CoverBackdrop>`), whereas email clients require `<table role="presentation">` and `cid:` MIME parts.
- Adding per-workspace IANA timezone settings or sub-day (hour/minute) gallery expiration times.
- Implementing `#349` scheduled expiration reminder jobs in this issue.

## Constraints

- `pp-mail` must compile and run its unit tests without `libvips` or PostgreSQL/`sqlx`.
- Keep zero free functions in non-test Rust code (`crates/AGENTS.md`).
- Never log email body text, recipient addresses, share URLs, access codes, or SMTP credentials (`crates/AGENTS.md`).

## Approaches

### Template engine in `pp-mail`
- **A. `askama` compile-time templates (chosen).** Real `.html` and `.txt` files, automatic HTML escaping (`HtmlEscaped` deleted), compile-time field/type checking during `cargo check`, and zero runtime overhead.
- **B. `maud` Rust macro DSL.** Avoids `.html` files and auto-escapes, but inline-styled Outlook `<table>` attributes inside Rust macros are harder to read and maintain, and plain-text emails still need separate string assembly.
- **C. `minijinja` runtime templates.** Clean template syntax, but template field typos fail at runtime/test time rather than `cargo check`.

### Server-side expiration calendar date
- **A. Plain PostgreSQL `DATE` (`YYYY-MM-DD`) valid through end of day UTC (chosen).** Matches `shoot_date` and `<DatePicker>` (`YYYY-MM-DD`), deletes `endOfLocalDay` / `toLocalDate` and `ExpiryBadge`'s hydration workaround, needs zero new routes or timezone crates, and guarantees identical calendar dates across studio, client badge, sent emails, and future `#349` reminders (`expires_at = CURRENT_DATE + 3`).
- **B. Workspace IANA timezone synced from browser + `chrono-tz`.** Preserves exact local midnight at the cost of a new workspace setting, API/schema plumbing, and `chrono-tz` dependency for sub-day precision photographers never configure.

## Decisions

- **Author's call — Date wording consistency:** Format `shoot_date` in emails as `12 Sep 2026` (EN) / `12 sept. 2026` (FR) and `expires_at` as `Available until 31 Oct 2026` (EN) / `Disponible jusqu’au 31 oct. 2026` (FR), matching `GallerySharePanel.svelte`'s `formatLongDate` output verbatim.
- **Author's call — Photo count in metadata line:** Always include `{count} photo(s)` (`1 photo` / `N photos` in EN; `0 photo` / `1 photo` / `N photos` in FR, matching `web/messages/{en,fr}.json` `summary_photos`) alongside `shoot_date` when present, matching `GallerySharePanel.svelte`'s `[shootDateText, countText].filter(Boolean).join(' · ')`.
- **Author's call — Migration `0036` ordering:** Base `#374` on top of `#375` ([#378](https://github.com/alekece/photo-proofing/pull/378)) so migration `0036_gallery_expires_at_date.sql` follows `0035_plain_gallery_access_code.sql` cleanly and `git mv crates/image-worker crates/worker` has no merge conflict.

## Left to the builder

- Exact module layout inside `crates/mail/src/` and `pp-core` placement of `Language`, `CoverSignature`, `FocalPoint`, and `InvitationStage`.
- Migration regression test for `0036_gallery_expires_at_date.sql`, unit tests in `pp-mail` verifying HTML escaping, access-code-above-CTA ordering, metadata line formatting in EN/FR, plain-text parity, and `cid:` MIME structure, plus updated `pp-database`, `pp-api`, `pp-worker`, and `web` tests.
