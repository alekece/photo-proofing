# Client selection Implementation Plan

> Use superpowers:executing-plans. Steps use checkboxes.

**Goal:** Clients favorite photos with a heart through the private link; the photographer sees them in the Favorites tab.

**Spec:** `docs/designs/2026-11-05-client-selection-design.md`

## Global Constraints

- `photos.favorite boolean NOT NULL DEFAULT false`; next free migration number after those on `main`.
- Every click saves immediately; no submit or confirmation.
- Writes need a valid share token, a ready photo of that gallery, and an undelivered gallery (`galleries.delivered_at IS NULL`, from #86).
- Overflow allowed; over the allowance the count is bold in the warning colour and the "extra" number stays in the text.

## Review Focus

- Cross-gallery, not-ready, expired, archived and delivered writes are rejected.
- Repeated and concurrent PUT/DELETE converge.
- Deleting a favorite photo asks for confirmation and removes only it.
- Failed save reverts the heart; reconnect refreshes the authoritative list.
- Heart is keyboard reachable with `aria-pressed` and an accessible name.

### Task 1: Storage and API

**Files:** new migration; `crates/database/src/photo*.rs`; `crates/api/src/routes/shared.rs` and `galleries.rs`.

**Produces:** repository `set_favorite(token/gallery, photo, bool)` and `favorites(gallery)`; routes `GET/PUT/DELETE /shared/{token}/favorites[/{photo_id}]`, `GET /api/v1/galleries/{id}/favorites`; a notification on change.

- [ ] Failing database and API tests for the Review Focus cases, then implement.
- [ ] fmt, clippy, test; commit.

### Task 2: Live updates

**Files:** the event hub and a per-link stream for clients; the photographer stream.

- [ ] Failing tests: a change in one browser reaches another on the same link; reconnect refetches the list.
- [ ] Implement via the existing Postgres-notification pattern; commit.

### Task 3: Client UI

**Files:** `ProofThumbnail.svelte`, `ProofViewer.svelte`, `ClientGallery.svelte`, schemas, server routes, tests.

- [ ] Failing tests: heart states, immediate save, revert on failure, counts and overflow styling, no hearts when delivered, disabled when `preview` is set (if #81 has landed).
- [ ] Implement; `pnpm --dir web verify`; commit.

### Task 4: Favorites tab and deletion confirmation

**Files:** `GalleryTabs.svelte`, photo deletion confirmation, tests.

- [ ] Failing tests: read-only list with "n of N included"; deleting a favorite asks for confirmation and keeps other favorites.
- [ ] Implement; verify; commit.
