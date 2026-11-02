# Simplified Photo Upload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the upload route read as access, validation, storage, atomic acceptance and response.

**Architecture:** Separate shared upload admission from `ApiState`; the extracted slot is only a capacity guard. Validation owns receiving and inspection, storage owns publication recovery, and `PhotoIntake` retains transaction/reconciliation guarantees.

**Tech Stack:** Rust, Actix Web, Tokio, libvips, PostgreSQL/SQLx.

**Spec:** [Approved upload design](../specs/2026-10-31-photo-upload-design.md), issue #98, PR #132.

## Global Constraints

- Preserve the existing HTTP contract, configuration defaults and unchanged original bytes.
- No request-body reads before authentication and gallery authorization.
- Hold capacity through acceptance/cleanup; reserve eagerly and fail fast with 503.
- Do not delete older originals or originals whose database acceptance is uncertain.
- Keep database retry policy unchanged; no worker, proxy or product-scope changes.
- Implement inline in the existing worktree, then obtain one fresh independent review.

## Review Focus

- Multiple worker applications must share one limit rather than each allocating capacity.
- Missing limiter application data must return an explicit infrastructure error.
- Cancellation/timeout must discard staged bytes and release reserved capacity.
- Failures before publication must not remove an existing original.
- Failed publication recovery must be reported, not disguised as successful cleanup.

---

### Task 1: Isolate upload admission

**Files:** Create `crates/api/src/upload/limiter.rs`; modify `crates/api/src/upload.rs`, `crates/api/src/api.rs`, `crates/api/src/routes/galleries/photos.rs`.

**Interfaces:** `UploadLimiter::new(NonZeroUsize) -> Self`; `UploadSlot: FromRequest` extracts from `Data<UploadLimiter>` and owns only its permit. `Api` registers the same limiter data across workers.

- [ ] Add tests extracting slots without `ApiState`: cloned data shares capacity, saturation returns 503, dropping a slot restores capacity, missing limiter returns 503.
- [ ] Run `cargo test -p pp-api --locked upload::limiter`; confirm the new interface is absent before implementation.
- [ ] Implement the limiter/extractor and server/test-app registration. Remove upload semaphore and acquisition from `ApiState`.
- [ ] Move body staging to a free helper in `upload.rs`; update interrupted-body/timeout tests without coupling a slot to state.
- [ ] Run admission, cancellation and authorization regressions; commit this coherent ownership change.

### Task 2: Make storage responsible for incomplete publication

**Files:** Modify `crates/storage/src/lib.rs` and `crates/api/src/upload.rs`.

**Interfaces:** `StagedPhoto::commit` retains its signature and write-once-original guarantee, but cleans its new original after failed directory sync. Explicit cleanup failure is a typed `StorageError`. `StagedPhotoExt` retains only `inspect`.

- [ ] Move pre-publication and post-publication regressions to direct storage calls; keep collision coverage. Add a cleanup-error contract test.
- [ ] Run `cargo test -p pp-storage --locked`; confirm failed durability currently leaves an original.
- [ ] Implement original-only recovery after the durability barrier fails; never remove a target on earlier failures or a derivative whose previous content was replaced. Report failed removal explicitly.
- [ ] Remove API `commit_original` and use storage `commit` with the standard HTTP error mapping. Update storage API docs/examples.
- [ ] Run storage and API upload regressions; commit the recovery-boundary change.

### Task 3: Express validation as one route step

**Files:** Modify `crates/api/src/upload.rs`, `crates/api/src/routes/galleries/photos.rs` and the design in place.

**Interfaces:** `validate_upload(&HttpRequest, Payload, &PhotoStorage, &UploadConfig) -> Result<ValidatedPhoto, ErrorResponse>` returns staged bytes, nonzero size and dimensions. It checks headers before staging, then streams bounded bytes and inspects JPEG metadata. Authorization stays in the route.

- [ ] Preserve route tests for wrong MIME/size, interrupted/empty/malformed JPEG, dimensions, timeout, gallery access, accepted metadata and unchanged bytes.
- [ ] Extract the existing header/stream/inspection sequence without changing errors or limits; route calls it after access checking, then storage and `PhotoIntake`.
- [ ] Mark the design current and completed plan steps in place; do not append refinement history.
- [ ] Run `cargo +nightly fmt --all --check`, `cargo test --workspace --locked` and `cargo clippy -p pp-api -p pp-database -p pp-storage --all-targets --locked -- -D warnings` with the existing PostgreSQL/libvips environment.
- [ ] Obtain one fresh independent review against the approved design; address material findings and rerun affected checks.
- [ ] Commit and push the existing PR branch. Retain the plan through PR review; do not merge.
