# Watermark Variants Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement in this session by default; use superpowers:subagent-driven-development only if the user chooses it. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make gallery watermark switches instant, keep photographer surfaces clean, and serve a clean client hero without exposing originals.

**Architecture:** Persist independent clean-pair readiness and marked-pair revision. Generate both pairs for each proof, update only marked files after settings changes, and choose derivative access policy on the server. Separate client-cover and authenticated client-preview routes from ordinary clean owner routes.

**Tech Stack:** Rust, PostgreSQL/SQLx durable jobs, libvips, Actix Web, SvelteKit/TypeScript, pnpm/Vitest.

**Spec:** [Approved design](../designs/2026-11-07-watermark-variants-design.md).
**Issue:** [#67](https://github.com/alekece/photo-proofing/issues/67), whose existing acceptance criteria cover watermark serving and approved regeneration behavior.
**PR:** [#231](https://github.com/alekece/photo-proofing/pull/231). Retained through review and corrections.

## Global Constraints

- Clean thumbnail: long edge at most 400 px; clean preview/hero: at most 1600 px; JPEG quality 75, no upscaling.
- Keep orientation, sRGB conversion and source-metadata stripping; originals never change.
- Four proof derivatives exist after initial publication, even without a configured watermark.
- Only the client gallery and authenticated client-gallery preview use marked derivatives.
- Ordinary owner surfaces and Settings samples always use clean derivatives.
- A toggle creates no jobs, file changes or photo-status changes.
- Marked-only updates cannot invalidate previously published clean copies.
- Client clean access when enabled is limited to the current hero preview, never arbitrary proofs or originals.
- Finals, selection ZIPs and original-download contracts remain unchanged.
- Use existing dependencies, method-owned Rust behavior, repository error handling and deterministic tests.
- This worktree is already isolated. Preserve unrelated edits and keep it through PR review.
- The app already renamed this branch before design creation; it permits only one rename. Do not bypass that restriction with raw Git to obtain the normal `feat/67-...` naming.

## Execution evidence

- Tasks 1-3 implemented in `0d32245`, `b7f8115`, and `a1cc92f`, with database,
  renderer/worker/storage and API regressions.
- Task 4 implemented with separate clean cover and client-preview media, persisted clean
  readiness, mounted-switch regression and usable clean tiles during marked failures.
- Required format/clippy/workspace tests and frontend verify passed. The Rust suites ran
  against isolated PostgreSQL 17 and libvips 8.15 because the host has unsupported libvips 8.12.
  Frontend verify covered 521 tests, type/lint/format checks and production build.
- Documentation uses the actual watermark requirement FR-07 (the original plan's FR-05
  reference was a numbering error); FR-03/04/05 clarify cover, visibility and processing.
- Browser verification passed with synthetic JPEGs through the real API/worker and frontend:
  clean owner tiles/hero with watermark enabled, marked client preview and shared gallery,
  clean Phone cover, switch-off clean previews, unchanged files/status/revisions/job count on
  toggle, and marked-only regeneration preserving both clean files and the original byte-for-byte.
  Paused regeneration showed truthful owner progress, clean disabled-client access and clean
  cover, while enabled client media returned 404.
- Independent review found a settings-save/upload-start race. A deterministic test reproduced
  the skipped upload reading revision 1 after revision 2 was saved. Regeneration now locks
  waiting proofs too, without resetting or adding jobs, so workers either read committed new
  settings or retain a replacement job. Workspace locking serializes new photo insertion
  with the scan, preventing phantom uploads from escaping it. Both ordered-scan interleavings
  and concurrent new insertion have regressions. Reviewer follow-up identified the same
  risk for manually retried failed photos; retry now takes the shared workspace lock before
  job/photo locks, with a deterministic old-revision regression.
- Dedicated refactor pass: access predicates and renderer preparation already share their
  owning helpers; separate serving routes enforce genuinely distinct policies. No additional
  abstraction or refactor was warranted beyond the corrected lock-scan responsibility.
  The same independent reviewer verified upload and retry corrections, with no remaining
  material issue. Final full Rust suites and frontend verify passed; an unchanged
  shutdown-grace timing test failed once under load, then passed both targeted and full reruns.
  Delivered for PR review; no merge has been performed.

## Review Focus

- A hero becomes hidden from the marked grid: keep its clean cover and owner images visible (Tasks 3 and 4).
- A disabled gallery is enabled during failed regeneration: hide stale marked images without enqueueing anything (Tasks 1 and 3).
- A pre-upgrade file contains a watermark: never recognize it as a clean derivative (Tasks 1 and 2).
- A client changes URL selectors or supplies a foreign photo id: deny clean/original bypasses (Task 3).
- A preview stays mounted across a switch: update its image identities without photo events; leave ordinary owner URLs clean (Task 4).

---

## File and interface map

Keep feature boundaries in existing modules rather than introduce a parallel image pipeline:

| Files | Responsibility |
|---|---|
| `crates/database/migrations/0017_proof_variants.sql` | Persist readiness and durable one-time preparation |
| `crates/database/src/{photo,watermark,jobs,gallery}.rs` | Access predicates, revision guards, publication and regeneration |
| `crates/storage/src/lib.rs` | Marked keys and all-variant deletion/orphan recognition |
| `crates/image-worker/src/render.rs`, `handlers/process_photo.rs` | Both-pair rendering and marked-only publication |
| `crates/api/src/routes/galleries/{photos,shared}.rs`, `galleries.rs`, `response.rs` | Authenticated clean media, client policy, cover and preview routes |
| `web/src/lib/gallery/{schemas,photos,proofs,types}.ts` | Readiness and separate image identities |
| `web/src/lib/components/{ClientGallery,ClientPreview,PhotosPanel,GalleryHeader,GalleryCard,WatermarkEditor}.svelte` | Correct image sources and regeneration feedback |
| `web/src/routes/g/[token]/{+page.server.ts,+page.svelte}`, `web/src/lib/server/galleries.ts` | Independent client cover URL |
| `docs/SPECIFICATIONS.md`, existing watermark design, `README.md` | Approved product policy and accurate current behavior |

Read a module's existing callers/tests before changing it. Migration number is based on the
current `0016` head; if another migration arrives first, choose the next unused number.
Tests stay in existing module test suites. Add focused web test files only where no suite exists.

### Task 1: Persist proof readiness and watermark-independent publication

**Files:** Migration above; database `photo.rs`, `watermark.rs`, `jobs.rs`, `gallery.rs`;
their existing inline tests.

**Interfaces:**
- Add `clean_ready: bool` and `watermark_revision: Option<i64>` to persisted proof metadata.
  `NULL` is unprepared, not revision zero. Expose `clean_ready` in owner `Photo` JSON.
- Add database enum `ProofAccess { Clean, Client }`, not a public bool.
- Add `PhotoRepository::list_visible(location: PhotoLocation, kind: PhotoKind, access: ProofAccess) -> Result<Vec<ReadyPhoto>, DatabaseError>`
  and `is_visible(location: PhotoLocation, id: PhotoId, kind: PhotoKind, access: ProofAccess) -> Result<bool, DatabaseError>`.
  Finals keep their existing status-based readiness.
- `Client` reads the stored switch: clean readiness when off, clean readiness plus current
  marked revision when on. `Clean` ignores the switch. Reuse this predicate for favorite writes.
- Keep `WatermarkStamp` but remove switch dependence from its SQL: every proof stamps the
  workspace revision, finals stamp `None`. Add `WatermarkStamp::revision(self) -> Option<i64>`.
- `ProofWatermark` also carries `clean_ready`; it returns the workspace watermark for every
  proof regardless of enablement, and no watermark for finals.
- Keep `begin_publication(&Job, PhotoId, WatermarkStamp)` and `OpenPublication::finish()`;
  retain the stamp in the publication guard and atomically persist readiness/revision at finish.
  Preserve the existing asset `revision` for published-image URL refresh.

- [ ] **Step 1: Write failing database contract tests.**
  `toggle_does_not_reset_or_enqueue`: snapshot job count, status and asset revision, toggle both ways,
  then `assert_eq!(after, before)` for all three.
  `settings_update_requeues_disabled_and_enabled_proofs`: assert both prepared proof sets
  become waiting, current marked access fails, clean access succeeds, and finals do not change.
  `switch_during_failed_marked_update`: assert off grants clean access, on denies client access,
  both keep the existing job count and failure reason.
  `publication_stamp_ignores_gallery_switch`: assert a toggle does not close a valid guard,
  but a workspace revision change does.
  `clean_ready_proof_remains_selectable_when_disabled`: assert favorite writes apply while
  marked copies are waiting/failed; enabling rejects a new selection but retains existing favorites.
- [ ] **Step 2: Run `cargo test -p pp-database watermark` and `cargo test -p pp-database photo`.**
  Expected: the new behavioral tests fail before implementation; SQLx tests require the repository test database.
- [ ] **Step 3: Add the migration and implement the interfaces and predicates.**
  Default old proofs to `clean_ready = false`, `watermark_revision = NULL`.
  Requeue old ready/processing proofs durably in the migration using the current process-photo
  payload and existing job-attempt policy. Existing waiting jobs suffice; old failed proofs stay
  unprepared and retain their manual retry. Never reset finals.
  `WatermarkRepository::update_gallery` delegates ordinary gallery update, without regeneration.
  Effective settings changes requeue all eligible workspace proofs, preserving clean readiness;
  waiting jobs read latest settings, failed photos retain retry behavior.
  Publication updates readiness only after valid guarded writes. Global status alone must not
  grant proof access. Shared hero lookup and hero assignment use clean readiness for proofs.
  Keep unrelated dashboard counts and export status policies unchanged.
- [ ] **Step 4: Add migration regression and concurrency tests, then rerun the database selectors.**
  Apply old/new migrations in a transaction-scoped schema. Assert old marked-at-old-key proofs
  are unprepared with durable jobs; finals and failure reasons are preserved.
  Assert duplicate/superseded jobs cannot publish a stale revision; changes during initial
  processing leave a latest-revision job. Extend existing publication-lock tests rather than sleep-based tests.
- [ ] **Step 5: Commit the database changes with `feat: track clean and marked proof readiness`.**

### Task 2: Generate both pairs and replace only marked copies

**Files:** Storage `lib.rs`, worker `render.rs`, `handlers/process_photo.rs`; inline tests.
**Consumes:** Task 1's stamp, `ProofWatermark.clean_ready` and publication guard.
**Produces:**
- Extend `Variant` with `WatermarkedThumbnail` and `WatermarkedPreview`, using explicit stable
  storage names `watermarked-thumbnail` and `watermarked-preview`.
- Keep `RenderedPhoto { thumbnail, preview }` for clean/final compatibility.
- Add `RenderedProof { clean: Option<RenderedPhoto>, watermarked: RenderedPhoto }`.
- Add `ProofRender { All, Watermarked }` and
  `ImageRenderer::render_proof(&self, original: &[u8], watermark: Option<&Watermark>, render: ProofRender) -> Result<RenderedProof, RenderError>`.
  Preserve `render` for finals; share bounded preparation and encoding internally.

- [ ] **Step 1: Write failing renderer and storage tests.**
  `proof_pairs_are_clean_and_marked`: with a nonempty mark, assert marked pixels differ from clean;
  both have the exact existing dimension/orientation/colour/metadata contracts.
  `unconfigured_watermark_produces_both_pairs`: assert the two pairs' encoded bytes are equal.
  `marked_only_render_has_no_clean_output`: assert `clean.is_none()` and valid marked outputs.
  `remove_many_removes_all_variants`: store all five files, remove and assert none exist.
  `sweeper_recognizes_marked_variants`: assert owned marked files remain and eligible orphans are removed.
- [ ] **Step 2: Run `cargo test -p pp-image-worker render` and `cargo test -p pp-storage`.**
  Expected: new tests fail before implementation; worker tests require supported libvips.
- [ ] **Step 3: Implement rendering and storage variants.**
  Resize each output once, encode clean and marked from its pixels; no compressed JPEG overlay.
  Without a mark, reuse encoded bytes, not a guessed watermark. Use `All` if unprepared and
  `Watermarked` if clean-ready; keep finals on existing two-output rendering.
  Stage the selected outputs, validate the claim and stamp under locks, rename, then finish.
  Marked-only jobs never overwrite clean files. Extend deletion and late-publication cleanup.
- [ ] **Step 4: Extend handler integration tests and run `cargo test -p pp-image-worker`.**
  Test enabled/disabled initial uploads produce four files, marked-only changes preserve clean
  byte hashes, and finals have only existing outputs. Exercise stale settings, expired claims,
  deletion during staging, partial publication failure and restart/retry using existing fixtures.
  A staged or renamed pre-upgrade file never changes clean readiness without successful finish.
- [ ] **Step 5: Commit with `feat: prepare clean and watermarked proof pairs`.**

### Task 3: Enforce distinct owner, client, cover and preview serving policies

**Files:** API `galleries.rs`, `galleries/photos.rs`, `galleries/shared.rs`, `response.rs`;
database access helpers from Task 1 as needed; existing route tests.
**Consumes:** Task 1's access predicates and Task 2's storage variants.
**Produces:**
- Existing owner `/{id}/photos/{photo_id}/thumbnail|preview`: always clean, using `Clean`.
- Existing shared photo routes: server chooses current variant using `Client`.
- New `GET /shared/{token}/cover`: current permitted hero's clean preview; no requested photo id.
- New authenticated `GET /{id}/client-preview/photos` and
  `GET /{id}/client-preview/photos/{photo_id}/{derivative:thumbnail|preview}`:
  client visibility/variant rules but session ownership, no share requirement.
- Add `PhotoRepository::visible_derivative(location, id, kind, access) -> Result<Option<ProofDerivative>, DatabaseError>`
  with enum `ProofDerivative { Clean, Watermarked }` to choose storage variants without exposing a selector.
  List and media authorization must use the same current policy.
- Add no-store proof response construction to `ImageFile` without changing final or original download policies.

- [ ] **Step 1: Write failing route tests.**
  Assert owner media stays clean for either toggle; shared media switches stored bytes.
  Assert a stale marked pair yields 404 without clean/original fallback.
  Assert shared cover serves clean bytes during marked regeneration even when the grid excludes
  its hero; cover takes no arbitrary photo id and honors expiry/unsharing/deletion/removal.
  Assert foreign/unprepared heroes, finals ids and variant/query traversal attempts do not bypass access.
  Assert client-preview routes require ownership, work before sharing and match client visibility.
- [ ] **Step 2: Run `cargo test -p pp-api routes::galleries`.**
  Expected: new route/policy tests fail before implementation.
- [ ] **Step 3: Implement the handlers and register literal routes ahead of dynamic ones.**
  Resolve gallery ownership/private-link access before readiness; choose variants from saved
  server policy, not the query string. Cover uses current hero and current allowed photo kind.
  Select variant/readiness in one repository read, not separate inconsistent policy reads.
  Never accept revision query parameters as authority to serve an older marked version.
  Use private/no-store for proof responses, preserving nosniff and no-referrer.
- [ ] **Step 4: Rerun route tests and add cache/favorite regressions.**
  Assert subsequent media requests after enablement do not receive clean bytes through a cached
  response. Assert settings failure leaves disabled clean selection working, and cover lookup
  no longer depends on global ready status. Preserve existing original/final/export endpoint tests.
- [ ] **Step 5: Commit with `feat: separate client watermark and clean owner media`.**

### Task 4: Wire clean owner surfaces and faithful client previews

**Files:** Web files in the interface map; existing `proofs.test.ts`, `schemas.test.ts`,
component tests, `routes/routes.test.ts`, `routes/g/[token]/page.server.test.ts`.
**Consumes:** New JSON `clean_ready`, client-preview routes and shared cover route.
**Produces:**
- Parse `clean_ready` into owner `Photo`; preserve status and failure reason independently.
- Add `coverUrl?: string` to `ClientGallery`, no longer derive cover from its grid proof list.
- Add `toClientPreviewProofs(galleryId: string, photos: SharedPhoto[], watermarkEnabled: boolean): Proof[]`
  using authenticated client-preview URLs with switch and asset revision in identity.
- Add a client-preview list request helper using existing request/error/schema infrastructure.

- [ ] **Step 1: Write failing tests for each image-consuming surface.**
  A ClientGallery whose hero is absent from `proofs` still renders supplied clean `coverUrl`.
  ClientPreview loads the distinct client-visible list and uses marked-policy media URLs.
  Changing the switch changes preview grid/viewer URL identity without a photo event.
  Dashboard, header, ordinary Photos/Favorites viewers and Settings use only clean owner URLs.
  Settings's sample overlay is applied once; stored marked URLs never enter it.
- [ ] **Step 2: Run `pnpm --dir web test` with the changed files' Vitest selectors.**
  Expected: new contract tests fail before implementation.
- [ ] **Step 3: Implement schemas, helpers and cover wiring.**
  Public page loader supplies the shared cover URL with hero identity; owner preview supplies
  the existing authenticated clean hero URL. Keep clean cover separate even when no marked
  proofs are currently visible: a known valid cover must not be discarded by the empty-grid branch.
  Keep empty-title/default-cover and delivery presentation behavior otherwise unchanged.
  Remove `toOwnerProofs` from client-preview consumption; fetch client-visible proofs and preserve
  favorites with the existing owner list or a scoped helper. No share-token shortcut.
- [ ] **Step 4: Implement clean-ready owner presentation and progress regressions.**
  PhotosPanel shows valid clean image/viewer/hero actions during marked-only waiting/failure,
  while reporting processing/failure and retaining retries. Do not falsely label regeneration
  as successful just because clean images exist. Replace `everReady` plus enabled-switch progress
  inference with persisted clean readiness, so disabled galleries and reconnects show work accurately.
  Ordinary owner image identities do not depend on toggles. On photo events, refresh readiness
  through current repository list data; existing events need not acquire a new type.
- [x] **Step 5: Run targeted tests, then `pnpm --dir web verify`.**
  Expected: zero format/lint/type warnings, all tests pass and build succeeds.
  Check desktop/phone cover and preview visually using synthetic photos and the browser canvas.
- [x] **Step 6: Commit with `feat: keep photographer images clean and preview client watermarking`.**

### Task 5: Documentation, end-to-end invariants and independent delivery review

**Files:** Approved design, existing watermark design, `docs/SPECIFICATIONS.md`, `README.md`;
existing integration suites only where whole-flow regressions are missing.
**Consumes:** Tasks 1-4; no new interfaces.

- [x] **Step 1: Update the approved product requirements and current implementation documents.**
  FR-03/FR-05 explain clean owner surfaces and hero, client-only marking, instant toggles and
  workspace-wide marked regeneration. Replace superseded single-pair/toggle-rebuild decisions
  in the existing watermark design; preserve unrelated settings/logo/rendering constraints.
  README describes one-time preparation and deployment sequencing: stop old API/workers before
  new migrations/code start, preserve volumes, and never allow old two-file workers afterward.
  Describe unavailable unprepared photos and explicit failure/manual retry, not silent backfill success.
- [x] **Step 2: Add a whole-flow regression using existing API/worker infrastructure.**
  Upload in an off gallery, complete processing, verify four files, switch on and off with unchanged
  job/status snapshots, change workspace watermark, and assert only marked bytes change.
  Verify clean owner images/hero and disabled selection remain available throughout.
- [x] **Step 3: Run the required affected-area checks.**
  `cargo +nightly fmt --all`
  `cargo clippy --workspace --all-targets --locked -- -D warnings`
  `cargo test --workspace`
  `pnpm --dir web verify`
  Configure the existing test database privately; do not print deployment secrets.
  Report missing prerequisites or failures; do not claim completion if required checks cannot run.
- [x] **Step 4: Request one fresh independent whole-branch review.**
  Review correctness and approved-spec compliance, especially clean-only owner routes, marked
  revision guards, backfill, selectors, cached images and all-variant cleanup. Fix material findings.
- [x] **Step 5: Perform a dedicated bounded refactor pass.**
  Remove duplicated access predicates or renderer preparation, clarify responsibilities/names,
  and record the result even if no change is warranted. Rerun affected checks after corrections.
- [x] **Step 6: Commit documentation and corrections, then deliver a PR referencing #67.**
  Use the repository's user credentials and no co-author trailer. Keep this plan through
  implementation/review; remove it only after corrections and before asking final merge approval.
  Never merge without explicit final user approval. Preserve the worktree for review.

## Plan self-review

The five tasks cover generation, toggle behavior, all-gallery regeneration, independent readiness,
access and selection, separate hero resolution, every image-consuming surface, settings previews,
cache identity, migration/restart/deletion and current documentation. Interface names agree across
tasks; rendering sizes and file policy stay those approved in the design. The five Review Focus
conditions each have an owning task test. No new dependencies, delivery policy or larger hero asset
is introduced.

## PR guidance corrections

A fresh independent guidance audit found missing separators in renderer/publication code,
shared-route request cases and adjacent frontend tests, plus incomplete readiness documentation
and stale gallery-screen regeneration guidance. Corrections preserve behavior and distinguish
initial upload status from regeneration over an already usable clean image.

The bounded refactor pass found no further responsibility or abstraction change warranted:
access policy and the authorized storage pair remain distinct, and worker redesign stays outside
this PR. The reported loop is alone in its async scope; the resize conditional already has a
separator. The app-managed branch was already renamed and cannot be renamed again in this session.

Verification after corrections: nightly format, strict workspace/all-target Clippy, all six
database doctests (including three new contract examples), and frontend verification
(529 tests in 71 files, zero type errors/warnings, production build) passed. No runtime behavior
changed, so database integration and libvips rendering suites were not repeated for these
spacing/documentation corrections.

The same fresh reviewer verified the correction diff, including tail-expression spacing, and
reported no material remaining guidance violation. All inline maintainer questions received
thread replies; representation/policy discussions remain open for the maintainer.

## Caller-chosen variant (supersedes earlier access interfaces)

Review feedback: the database should not enforce the saved policy; the caller decides what it
wants. `ProofAccess`, `ProofDerivative`, `list_visible`, `is_visible`, `visible_derivative` and the
authenticated `client-preview` routes are replaced by a single `Watermarked(bool)` newtype.
`list_ready`, `is_ready` and `set_favorite` take it and only check file readiness (marked means the
current workspace revision). Owner `photos` list/thumbnail/preview routes accept `?watermark=true|false`
(default clean); the client preview requests the variant matching the gallery switch. Public shared
routes derive the variant from the saved switch and never accept a selector.

Verification: full workspace tests against PostgreSQL 17 and libvips 8.15, strict Clippy, and
frontend verification passed.
