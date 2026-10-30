# Gallery creation and configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Photographers create, list, reopen and autosave-edit galleries (#64).

**Architecture:** `galleries` table and `GalleryRepository` in `pp-database`; `/api/v1/galleries` routes in `pp-api` behind an `Identity` extractor and the generalised `Origin` check, with a generic `ErrorResponse`; SvelteKit dashboard and gallery page using a reusable `Placeholder`.

**Tech Stack:** Rust (actix-web, sqlx, serde_with), SvelteKit 5, Tailwind, vitest.

**Spec:** `docs/superpowers/specs/2026-10-30-gallery-configuration-design.md`

## Global Constraints

- Title trimmed, 1-255 characters (database CHECK); `included_photos` null or at least 1.
- Defaults: title "Untitled gallery", layout `grid`, no expiry, unlimited photos, watermark off.
- PATCH: missing key keeps, `null` clears optional settings only; `null` on `title`, `layout`, `watermark_enabled`, unknown keys and wrong types are 422.
- Every query scoped by `workspace_id`; foreign galleries are 404.
- Sorts: `updated` (default), `created` newest first, `title` ascending case-insensitive; tiebreak on `id`.
- Rust: AGENTS.md style, `cargo +nightly fmt --all`, clippy `-D warnings`.

## Review Focus

- Clearing `expires_at` or `included_photos` with `null` must not touch other fields (tested in database and API).
- Another workspace's gallery id returns 404 on GET and PATCH.
- Whitespace-only and 256-character titles are rejected; 255 accepted.
- `included_photos: 0` and beyond `i32::MAX` rejected, never wrapped.
- Gallery routes reject missing or foreign `Origin` and missing session.

## Task 1: Database (`crates/database`)

**Files:** create `migrations/0002_galleries.sql`, `src/gallery.rs`; modify `src/lib.rs`, `Cargo.toml`.

**Produces:** `GalleryId`, `Layout`, `Timestamp`, `Gallery`, `GalleryChanges`, `GallerySort`, `GalleryError`, `GalleryRepository` with `create`, `list`, `get`, `update` (see design).

- [ ] Tests first (`#[sqlx::test]`): defaults, ownership, Keep/Set/Clear, title and count CHECK mapped to `GalleryError::Invalid`, three sort orders, empty changeset leaves `updated_at`.
- [ ] Implement; run `cargo test -p pp-database`.
- [ ] Commit.

## Task 2: API (`crates/api`)

**Files:** create `src/error.rs`, `src/origin.rs`, `src/extractors/identity.rs`, `src/routes/galleries.rs`; modify `routes/auth.rs`, `routes.rs`, `extractors.rs`, `api.rs`, `lib.rs`, `Cargo.toml`, workspace `Cargo.toml`.

**Interfaces:** `ErrorResponse` (`From<AuthError>`), `require_same_origin` shared, `Authenticated(Identity)` extractor.

- [ ] Tests first (actix `test`): auth and origin required, create 201, list sorts, patch set/clear/422 cases, cross-workspace 404.
- [ ] Implement; run `cargo test -p pp-api`.
- [ ] Commit.

## Task 3: Web (`web/`)

**Files:** `src/lib/components/Placeholder.svelte`, `src/lib/gallery/api.ts`, `src/lib/server/galleries.ts`, dashboard page, `src/routes/dashboard/[id]/...`, tests.

- [ ] `Placeholder` (`background: 'sheet' | 'gradient'`, optional `icon`/`label`, `loading` pulse with `prefers-reduced-motion`).
- [ ] Dashboard list, sort, New gallery (disabled in flight), empty/loading/error states.
- [ ] Gallery page: inline title, settings panel, autosave states, date picker with end-of-day local conversion, retry.
- [ ] `pnpm test`, `pnpm check`. Commit.

## Task 4: Delivery

- [ ] fmt, clippy, tests; README/docs update; remove this plan; PR with open questions as comments.
