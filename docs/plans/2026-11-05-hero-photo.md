# Hero photo Implementation Plan

> Use superpowers:executing-plans. Steps use checkboxes.

**Goal:** Star a ready photo as the gallery hero; show its preview on the dashboard card and the client cover.

**Spec:** `docs/designs/2026-11-05-hero-photo-design.md`

## Global Constraints

- Preview derivative only, never the original.
- Hero must be a ready photo of the same gallery and workspace; deleting it clears the hero (no fallback).
- A star click saves immediately.
- Next free migration number after those on `main` (check at implementation time; #86 uses 0008).

## Review Focus

- Foreign-gallery, foreign-workspace and non-ready photo ids are rejected.
- Deleting the hero photo or its gallery leaves no dangling reference.
- Failed save keeps the previous hero in the UI.
- Star is keyboard reachable and has an accessible name and `aria-pressed`.
- A hero that is no longer ready is not exposed to clients.

### Task 1: Storage and update (database, API)

**Files:** new migration adding `galleries.hero_photo_id uuid REFERENCES photos(id) ON DELETE SET NULL`; `crates/database/src/gallery.rs`; `crates/api/src/routes/galleries.rs` (and `shared` for the client payload).

**Produces:** `Gallery.hero_photo_id: Option<PhotoId>`; `GalleryChanges.hero_photo_id: Option<Option<PhotoId>>` (double-option like `expires_at`); shared payload `hero_photo_id` only when that photo is ready.

- [ ] Write failing database tests (set, change, clear, foreign gallery, foreign workspace, non-ready, cleared on photo deletion, change after share).
- [ ] Implement; validate the photo inside the update query (same gallery, workspace, status ready); unknown or invalid photo gives the same not-found error as other missing resources.
- [ ] Write failing API tests (PATCH round trip, rejections, shared payload) then make them pass.
- [ ] Format, clippy, test, commit.

### Task 2: Web data and star toggle

**Files:** `web/src/lib/gallery/schemas.ts`, `api.ts`, `PhotoTile.svelte`, `PhotosPanel.svelte` and their tests.

**Produces:** `hero_photo_id` (nullable) in the gallery and shared schemas; `PhotoTile` props `hero?: boolean` and `onhero?: () => void`; the panel calls the existing gallery update with the id or `null`.

- [ ] Failing tests: star states and accessible names ("Set as hero photo" / "Remove hero photo", `aria-pressed`), one click saves, failure keeps the old hero and shows an error.
- [ ] Implement: star on ready tiles only, top right, visible on hover/focus-visible, always on touch and on the hero.
- [ ] `pnpm --dir web verify`, commit.

### Task 3: Dashboard card, client cover, settings hint

**Files:** `web/src/routes/dashboard/+page.svelte`, `ClientGallery.svelte`, `GallerySettings.svelte`, README or docs if they mention the hero, and tests.

- [ ] Failing tests: card and cover use the preview URL; no hero shows the placeholder / no cover.
- [ ] Implement; replace the settings placeholder text with a hint to star a photo in the Photos tab.
- [ ] `pnpm --dir web verify`, commit.
