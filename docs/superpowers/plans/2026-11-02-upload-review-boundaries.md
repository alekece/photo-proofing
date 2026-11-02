# Upload Review Boundaries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Address the latest #132 review with explicit upload ownership and simpler route cleanup.

**Architecture:** `PhotoUpload::from_request` receives into storage staging after authorization;
`upload/jpeg.rs` owns image initialization/inspection. The route retains publication and
explicit database-outcome cleanup. UUID identifiers own generation through `Generate`.

**Tech Stack:** Rust, Actix Web, Tokio, libvips and SQLx.

**Spec:** [Approved design](../specs/2026-10-31-photo-upload-design.md), #98 and #132.

## Global Constraints

- Preserve disk-backed bounded receiving, header checks before staging, no body polling before access checks.
- Keep HTTP responses, original bytes, filename decoding and already-removed cleanup semantics.
- Keep database atomicity, bounded retry and uncertain-commit preservation.
- Missing limiter and saturation both remain 503, as confirmed in review discussion.
- Existing worktree and branch; inline implementation, one fresh review, then a dedicated refactor pass.
- No new dependencies or change to ID representations.

## Review Focus

- Constructor is called after authorization rather than as an eager body-consuming extractor.
- JPEG inspection stays off the async runtime and rejects invalid headers/dimensions.
- Cleanup never deletes originals for an unknown database outcome.
- Storage errors are sanitized, logged and preserve storage-full responses.
- Refactoring preserves filename fallback, byte limits, idle timeout and cancellation cleanup.

---

### Task 1: Identifier generation

**Files:** `crates/core/src/types.rs`, new `crates/core/src/types/generate.rs`,
`crates/core/src/notifications.rs`, `crates/database/src/{auth,gallery,jobs}.rs`,
`crates/api/src/routes/galleries/photos.rs`.

**Interface:** `Generate::generate() -> Self`, implemented explicitly for UUID-backed IDs.

- [x] Add a generic capability test for PhotoId and a public runnable example using only the ID type.
- [x] Run `cargo test -p pp-core --locked`; expect absent capability before implementation.
- [x] Implement trait and ID implementations; migrate production ID creation in auth/gallery/jobs/upload, preserving conversions and serialization.
- [x] Run core/database tests and commit with the approved design.

### Task 2: Upload and JPEG boundaries

**Files:** `crates/api/src/upload.rs`, new `crates/api/src/upload/jpeg.rs`,
`crates/api/src/{api,error}.rs`, `crates/api/src/routes/galleries/photos.rs`.

**Interfaces:** `PhotoUpload::from_request(&HttpRequest, Payload, &PhotoStorage, &UploadConfig)
-> Result<Self, ErrorResponse>` returns staged file, size, dimensions and original filename.
`jpeg::install()` initializes bounded libvips; `jpeg::inspect(&Path, NonZeroU64)` returns
dimensions. `From<StorageError> for ErrorResponse` logs/sanitizes failures.

- [x] Adapt constructor tests first; run API tests expecting missing constructor.
- [x] Move JPEG initialization/inspection and its tests; delete StagedPhotoExt rather than add another wrapper.
- [x] Implement constructor; use `photo_upload` in the route and move filename collection into it.
- [x] Replace storage_failure with standard error conversion; remove upload cleanup helpers.
- [x] Keep explicit Database/CommitOutcomeUnknown branches in route. Direct storage.remove treats NotFound as already removed; all other errors propagate.
- [x] Move unknown-commit preservation regression to the route-owned cleanup decision without adding production test-only injection.
- [x] Add storage conversion regression; preserve auth/header/cancellation/cleanup/JPEG route tests.
- [x] Add missing whitespace after storage initialization; run API tests and commit.

### Task 3: Fresh review, then refactor pass

**Files:** Changed production/tests/design plus `AGENTS.md`.

- [x] Update existing design in place and document the user-directed post-review refactor step in repository guidance.
- [ ] Run nightly fmt check, workspace tests and API/database/storage Clippy with warnings denied.
- [ ] Obtain one fresh independent review of correctness, boundaries and repository-guidance compliance; address material findings.
- [ ] Perform a distinct inline refactor pass after that review: inspect naming, responsibility boundaries, redundant wrappers, visibility, imports/spacing and tests. Make only evident behavior-preserving improvements within approved scope; record what changed or why none was needed.
- [ ] Re-run affected checks after refactoring, commit, push #132, reply `[agent]` in each new review thread; leave threads open pending maintainer review. Never merge.
