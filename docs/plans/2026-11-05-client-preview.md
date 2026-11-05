# Client preview Implementation Plan

> Use superpowers:executing-plans. Steps use checkboxes.

**Goal:** Owner-only full-screen preview of the client page with Desktop and Mobile views.

**Spec:** `docs/designs/2026-11-05-client-preview-design.md`

## Global Constraints

- No new route or endpoint; reuse the gallery and photos the page already loaded.
- Preview derivatives only, never originals; never creates a share link.
- Selection and download controls visible but disabled; "Preview" marker shown.
- Mobile frame about 390px wide, fixed height, scrolls inside the overlay.

## Review Focus

- Unshared, expired and empty galleries still preview.
- Esc and Close dismiss; focus is trapped and returns to the Preview button.
- The shared page without the flag is unchanged.
- Viewer opens inside the frame and does not escape it in Mobile view.
- Toggle is keyboard operable with accessible names and `aria-pressed`.

### Task 1: `preview` flag and proof mapper

**Files:** `web/src/lib/components/ClientGallery.svelte`, `ProofGrid.svelte`/`ProofThumbnail.svelte`/`ProofViewer.svelte` as needed, a mapper in `web/src/lib/gallery/` (owner ready photos to `Proof[]` with owner preview URLs); tests.

**Produces:** `ClientGallery` prop `preview?: boolean`; `toProofs(photos, galleryId): Proof[]`.

- [ ] Failing tests: marker shown and write controls disabled when `preview`, unchanged otherwise; mapper keeps only ready photos and uses preview URLs.
- [ ] Implement; run `pnpm --dir web verify`; commit.

### Task 2: Overlay and Preview button

**Files:** new `web/src/lib/components/ClientPreview.svelte` (+ test), gallery page that hosts the tabs, `GalleryTabs.svelte` if the button lives there.

**Produces:** `ClientPreview` props `gallery`, `photos`, `onclose`.

- [ ] Failing tests: opens for unshared, expired and empty galleries; Desktop/Mobile toggle; Esc and Close dismiss and return focus; focus trap; no share link created.
- [ ] Implement full-screen labelled modal, header (marker, toggle, Close), scrollable ~390px frame for Mobile.
- [ ] `pnpm --dir web verify`; commit.

### Task 3: Product documents

**Files:** `docs/SPECIFICATIONS.md`, `docs/releases/MVP.md`.

- [ ] Add the short requirement and MVP scope/acceptance line described in the design; keep claims aligned with the code. Commit.
