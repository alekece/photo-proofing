# Staged Photo Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement inline. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Address the third review of PR #132 without changing upload behavior.

**Architecture:** ApiState owns upload resources directly. API-local extension methods on
storage's StagedPhoto handle inspection and original publication; storage stays policy-free.

**Tech Stack:** Rust, Actix Web, SQLx, Tokio, libvips.

**Spec:** `docs/superpowers/specs/2026-10-31-photo-upload-design.md`, approved third refinement.
**Issue:** #98. **PR:** #132. Use the existing isolated worktree.

## Global Constraints

- Preserve eager admission, authorization before body consumption and permit lifetime.
- Preserve collision safety, uncertain-commit preservation and transaction atomicity.
- No dependencies, schema, product scope, or behavior changes.
- Keep free upload::install and prefix GitHub replies with `[agent]`.

## Review Focus

- Inspection rejection and cancelled requests release staging files and admission permits.
- Publication collisions preserve the pre-existing original; partial publication is cleaned.
- Nested module registration preserves URLs and authorization.
- Flattened state still shares a single semaphore across server workers.
- Derived SQLx conversion retains sanitization and never replaces commit-outcome classification.

### Task 1: Staged-photo methods and direct application resources

**Files:** `crates/api/src/{api.rs,upload.rs,tests.rs,routes.rs}`,
`crates/api/src/routes/{galleries.rs,photos.rs}`,
new `crates/api/src/routes/galleries/photos.rs`,
`crates/api/src/middlewares/failed_attempts.rs`.

**Interfaces:**
- ApiState::new additionally accepts PhotoStorage and UploadConfig instead of UploadState.
- ApiState::storage() -> &PhotoStorage; upload_config() -> &UploadConfig;
  try_acquire_upload_slot() -> Option<OwnedSemaphorePermit>.
- StagedPhotoExt::inspect(&self, max_pixels: NonZeroU64) -> Result<PhotoDimensions, ErrorResponse>.
- StagedPhotoExt::commit_original(self, storage: &PhotoStorage, key: &PhotoKey)
  -> Result<(), ErrorResponse>.
- upload::remove_original(&PhotoStorage, &PhotoKey) -> Result<(), ErrorResponse>.
- upload::cleanup(&PhotoStorage, &PhotoKey, &PhotoIntakeError) -> Result<(), ErrorResponse>.
- upload::original_name(&HttpRequest) -> String; storage_failure(StorageError) -> ErrorResponse.
- tests::upload_resources() -> (PhotoStorage, UploadConfig, TempDir), with callers retaining TempDir.

- [ ] Adapt upload unit tests to the extension methods and standalone cleanup helpers; retain
  collision preservation, uncertain commits, pixel overflow, timeout and interrupted-body checks.
  Add explicit successful staged inspection/publication coverage if not already covered by routes.
- [ ] Run `cargo test -p pp-api --locked --lib`; expect missing interfaces before implementation.
- [ ] Implement direct resources and extension methods, keeping storage untouched. Adapt
  fixtures and route calls. Move photos into the galleries child module, update registration
  and test paths. Qualify the middleware helper as `tests::upload_resources`.
- [ ] Run `cargo test -p pp-api --locked --lib`; expect all tests green, including route
  authorization, saturation/cancellation, JPEG rejection and original publication.
- [ ] Commit as `refactor: attach upload operations to staged photos` with co-author trailer.

### Task 2: Conversion simplification and inference answer

**Files:** `crates/database/src/photo_intake.rs`; inference probe in session scratch only.

**Interfaces:** derive_more::From generates SQLx and DatabaseError conversions into the
Database variant, through DatabaseError's sanitization; from_commit stays unchanged.

- [ ] Add `sqlx_conversion_preserves_sanitization_and_source` testing a configuration error
  containing a sentinel secret: no displayed cause leaks it; the cause remains DatabaseError.
- [ ] Run `cargo test -p pp-database --locked photo`; the new test may already pass because this
  is behavior-preserving conversion cleanup. Record that fact rather than fabricating a RED.
- [ ] Use existing derive_more::From with explicit input types and remove the manual impl.
  Compile a minimal temporary Rust inference probe matching repository().authenticate(); if
  rustc reports E0282, retain explicit types and explain the requirement in the thread.
- [ ] Run `cargo test -p pp-database --locked photo`; expect all tests green.
- [ ] Commit as `refactor: derive photo intake error conversions` with co-author trailer.

## Final delivery

- [ ] Run nightly formatting check, `cargo test --workspace --locked` and
  `cargo clippy -p pp-api -p pp-database --all-targets --locked -- -D warnings`.
- [ ] Obtain one fresh independent review against this refinement; correct material findings.
- [ ] Push to PR #132 and reply to all eight new threads. Leave merge approval to the maintainer.
- [ ] Preserve design, plans and worktree through PR review; do not merge.

Database-backed commands use DATABASE_URL=postgres://postgres@127.0.0.1:5432/photo_proofing_test.
API commands also use LIBRARY_PATH and LD_LIBRARY_PATH pointing to
/tmp/photo-proofing-libvips/root/usr/lib/x86_64-linux-gnu.
