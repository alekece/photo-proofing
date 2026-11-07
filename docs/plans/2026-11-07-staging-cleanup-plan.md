# Staging Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans
> to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Address the fourteen staging observations and existing #224 gallery polish.

**Architecture:** Refine the existing Svelte components and their async state.
Extend authenticated original downloads and the existing ZIP snapshot, preserving
one active preparation slot. Reproduce watermark freshness before changing its
publication or image-version boundary.

**Tech Stack:** Svelte 5, SvelteKit, Tailwind, Vitest, Actix, SQLx, PostgreSQL.

**Spec:** `docs/designs/2026-11-07-staging-cleanup-design.md`, issue #224.

**Execution:** Tasks 1–5 and implementation review are complete. Overlapping
component/API changes are delivered in one coherent commit. Deliver fetches
the current finals list directly, rather than adding a redundant count prop,
so its count, warning and photo stack always describe the same set.
All material independent-review findings are corrected; the dedicated
refactor pass warranted no further extraction. Final checks passed 513
frontend tests, 404 Rust tests and five doctests; browser checks verified
no edit reloads and first-viewer watermark freshness. PR delivery, CI and
temporary-plan cleanup remain the final gate.

## Global Constraints

- Preserve warm-paper styling and existing access rules; no new dependencies by default.
- Remove Open; retain read-only Preview and Copy link.
- Keep delivery, OTP, email/SMS, notifications and quota changes outside scope.
- Use `Download`, disabled `Downloading...`, and disabled `Saving...`.
- Originals and finals remain unwatermarked; do not bypass client access checks.
- No silent failures, arbitrary URL timestamps, full-page reload workarounds or weakened tests.

## Review Focus

- Slow responses and double clicks produce one request, with visible busy/error state.
- Rapid saves and SSE updates never reset loaded panels or overwrite newer edits.
- Zero finals with many proofs still reports zero in Deliver.
- Concurrent whole/subset ZIP requests do not cancel an active preparation.
- Signature logos and originals never leak through unavailable or foreign galleries.

### Task 1: Stable gallery editing and accurate counts

**Files:** `web/src/routes/dashboard/[id]/+page.svelte`, its `page.test.ts`,
`web/src/lib/components/GalleryTabs.svelte`, `GalleryHeader.svelte`,
`GallerySettings.svelte`, `PhotosPanel.svelte`, `PhotosPanel.test.ts`.

**Interfaces:** Expose `finalCount?: number | null` as a bindable GalleryTabs prop;
use this count for PublishDialog delivery mode. Preserve `photoCount` for sharing.
Pass `shared: boolean` from the page's current share token to GalleryTabs.
Extend Tabs items with `disabled?: boolean`; keyboard navigation skips disabled
items and cannot select them. Favorites and Finals are disabled until shared.

- [x] Add regressions for no Open shortcut, Preview retained, zero/different finals
  counts, no Saved text/toast on rapid autosaves, and panels/viewer not resetting
  after unrelated title/layout saves. Preserve existing title-failure retries.
- [x] Run targeted Vitest tests and confirm the new assertions fail.
- [x] Reproduce the blink with effect/request instrumentation; fix tracked
  dependencies or state updates that clear/remount panels. Remove success-only
  autosave notifications, retain pending/error announcements, wire finalCount.
- [x] Run the same targeted tests; expect all pass.
- [x] Update gallery/preview screen docs and commit this bounded change.

### Task 2: Pending controls, upload presentation and gallery select mode

**Files:** `PhotoTile.svelte`, `PhotoTile.test.ts`, `PhotosPanel.svelte`,
`PhotosPanel.test.ts`, `GalleryHeader.svelte`, `GalleryHeader.test.ts`,
`ui/confirm-dialog/confirm-dialog.svelte`, `ui/photo-stack/photo-stack.svelte`,
`web/src/routes/settings/+page.svelte`, `web/src/routes/settings/page.test.ts`.

**Interfaces:** Preserve ConfirmDialog's existing callback contract and pending
state; use the existing photo deletion and Settings save functions.

- [x] Add tests for delayed delete/save responses (busy state and one request
  after two clicks), rejection returning to retryable state, and upload status
  staying within the correct tile. Assert select mode keeps its toolbar.
- [x] Run targeted Vitest tests and confirm failing regressions.
- [x] Correct status placement; animate pending confirmation and keep deletion
  shrink/fade; add Settings Saving... state; center journey steps and remove the
  dark select-mode replacement. Preserve reduced-motion behavior.
- [x] Run targeted tests and inspect pending controls with keyboard focus.
- [x] Update gallery/settings screen docs and commit.

### Task 3: Original and subset downloads with quiet progress

**Files:** `crates/api/src/routes/galleries/photos.rs`, `export.rs`,
`crates/database/src/export.rs`, `crates/api/src/error.rs`,
`web/src/lib/gallery/export.ts`, `photos.ts`, `schemas.ts`,
`web/src/lib/components/FavoritesPanel.svelte`, `FavoritesPanel.test.ts`,
`GalleryTabs.svelte`. Rust regressions live in the modules' test sections.

**Interfaces:** Add owner-authenticated
`GET /api/v1/galleries/{gallery_id}/photos/{photo_id}/download` with attachment
response. Extend `POST .../export` with optional `{ photo_ids: string[] }`:
omission means all favorites; a present nonempty list is the explicit favorite
proof subset. Extend `requestExport` without breaking fetcher injection in tests.
Use existing persisted export entries as the worker snapshot.

- [x] Add API/database tests for one original, wrong workspace/gallery, client
  denial, empty/duplicate/foreign/nonfavorite/final subset IDs, unchanged whole
  selection, and competing ZIP requests. Define duplicate IDs as invalid.
- [x] Add Favorites tests for hover/focus download, subset selection, disabled
  Downloading... before POST resolves, through preparing and until initiation;
  failure offers retry and no separate download-started success text appears.
- [x] Run targeted tests and confirm expected failures.
- [x] Stream individual originals through scoped storage and owner authentication.
  Extend snapshot creation for subsets, reject a competing active export with a
  typed conflict, retain limits/retention. Wire tile/viewer/subset/whole actions
  and request-pending state. Do not allocate whole original files in memory.
- [x] Run affected Rust/Vitest tests; update README and Favorites API/UI docs;
  commit. Verify worker ZIP entries equal the requested snapshot.

### Task 4: Cover signature and consistent empty states

**Files:** `StudioSignature.svelte`, `StudioSignature.test.ts`,
`ClientGallery.svelte`, `ClientGallery.test.ts`, `ClientPreview.svelte`,
`FavoritesPanel.svelte`, `PhotosPanel.svelte`, `ui/empty-state/empty-state.svelte`,
`crates/api/src/routes/galleries/shared.rs`, `web/src/lib/gallery/schemas.ts`,
`types.ts`, `web/src/routes/g/[token]/+page.svelte`.

**Interfaces:** Extend the shared-gallery response with only the selected public
signature fields. Serve normalized logo via a dedicated shared route requiring
`ClientAccess`; Preview reads owner Settings via its existing authenticated API.

- [x] Add tests for name/logo/both/none, centered logo above text, client and
  Preview parity, unavailable-link logo rejection and identical empty strips.
- [x] Run targeted Rust/Vitest tests and confirm expected failures.
- [x] Wire signature metadata/normalized logo access, preserve unavailable-link
  responses, and share existing empty-state presentation across all three tabs.
- [x] Run targeted tests and inspect desktop/mobile signature and empty states.
- [x] Update client/preview/gallery docs and commit.

### Task 5: Current watermark assets and anchored dashboard controls

**Files:** `PhotosPanel.svelte`, `FavoritesPanel.svelte`, `ProofViewer.svelte`,
`web/src/lib/gallery/photos.ts`, `proofs.ts`, `GalleryStateFilter.svelte`,
`GalleryStateFilter.test.ts`, `web/src/routes/dashboard/+page.svelte`.
If reproduction requires revision propagation: `crates/database/src/photo.rs`,
`watermark.rs`, `jobs.rs`, `crates/image-worker/src/handlers/process_photo.rs`,
`crates/api/src/routes/galleries/photos.rs`, shared response schemas and one
new migration after checking current main's highest version.

**Interfaces:** Preserve existing asset URL helpers; if needed add an optional
server-rendered revision parameter propagated consistently to tiles/viewer/hero.

- [x] Reproduce first-photo staleness with network/cache and worker-event evidence.
  Add a regression at the failing boundary, including ready-to-regenerated-ready,
  an already-open first viewer image, and finals/original exclusion.
- [x] Add dashboard tests for fixed trigger size/right alignment across long
  selections and a visible nonblack search focus indicator.
- [x] Run targeted tests and confirm failing assertions.
- [x] Fix the demonstrated freshness boundary; use persisted render revision only
  if existing metadata is insufficient. Anchor the status popover to the right,
  constrain its trigger label, and use incumbent focus tokens.
- [x] Run affected checks, update docs, and commit.

### Task 6: Delivery evidence and independent review

- [x] Run `pnpm --dir web verify`, `cargo +nightly fmt --all`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `cargo test --workspace` with the repository's database/test environment.
  Record actual blockers; do not assume new libvips failures are baseline.
- [x] Capture desktop and 390 px before/after PNGs for upload, pending delete,
  signature, empty tabs, dashboard focus/menu and saving/downloading states.
- [x] Request one fresh independent review of correctness and design compliance.
- [x] Address material findings, perform a bounded refactor pass and rerun checks.
- [ ] Preserve plan during PR corrections, remove it only before final merge
  approval, and deliver a PR referencing #224 with evidence and limitations.
