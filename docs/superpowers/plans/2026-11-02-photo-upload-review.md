# Photo Upload Review Corrections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the PR review simplifications while keeping upload security and photo/job atomicity intact.

**Architecture:** Keep `UploadState` responsible for upload resources and its private pipeline methods, with Actix handlers remaining route functions. Make `PhotoStatus::Failed(String)` carry the API failure reason, while PostgreSQL retains separate status and reason columns constrained to stay consistent. A `PhotoIntake` coordinator owns one transaction and composes focused photo and job persistence methods.

**Tech Stack:** Rust 2024, Actix Web, SQLx/PostgreSQL, Tokio, libvips, `pp-storage`.

**Spec:** `docs/superpowers/specs/2026-10-31-photo-upload-design.md` (approved, including the PR review refinement)

## Global Constraints

- One raw `image/jpeg` request body contains one file; parallel per-file requests form a batch.
- Default limits remain 64 MiB per file, 100,000,000 pixels, and four concurrent uploads; existing TOML sections and environment variable names remain unchanged.
- Reject declared and streamed oversize bodies, malformed/truncated JPEG headers, and excessive dimensions before publishing the original.
- Preserve accepted original bytes unchanged; filenames remain display metadata only.
- The persisted photo and processing job remain atomic; unresolved commit outcomes must preserve the published original.
- List all four processing states in repository order; failed states carry their failure reason in `PhotoStatus`.
- Upload UI, image processing, derivatives, client delivery, and live updates remain out of scope.

## Review Focus

- A JPEG truncated inside the SOS header returns 422 and leaves no staged file, original, photo, or job — Task 1 keeps this regression test.
- A stalled body returns 408 and drops the staged file; exiting the handler releases its upload slot — Task 1 retains the timeout cleanup test and verifies permit release.
- A failed database commit whose outcome cannot be confirmed never causes deletion of a possibly committed original — Task 3 tests idempotent reconciliation and preservation.
- A gallery deleted between authorization and insertion yields no photo/job and removes the just-published original — Task 3 tests route cleanup.
- A failed photo without a failure reason, or another status with a failure reason, is rejected by the database invariant — Task 2 tests both constraint cases.

---

### Task 1: Simplify and rename upload API types

**Files:**
- Move: `crates/api/src/photo_upload.rs` to `crates/api/src/upload.rs`
- Modify: `crates/api/src/config.rs`
- Modify: `crates/api/src/api.rs`
- Modify: `crates/api/src/extractors/identity.rs`
- Modify: `crates/api/src/lib.rs`
- Modify: `crates/api/src/routes/auth.rs`
- Modify: `crates/api/src/routes/photos.rs`
- Test: `crates/api/src/config.rs`
- Test: `crates/api/src/upload.rs`
- Test: `crates/api/src/routes/photos.rs`

**Interfaces:**
- Produces `config::UploadConfig`, retaining the existing `ApiConfig::photo_upload` field, TOML section, and `PP_PHOTO_UPLOAD_*` environment variables. Its `max_size` and `max_pixels` fields use `NonZeroU64`; `concurrency` remains `NonZeroUsize`. Remove `UploadSize`, `PixelLimit`, and `ParseUploadLimitError`.
- Produces `upload::UploadState`, with `install(PhotoStorage, UploadConfig) -> Result<Self, PhotoUploadError>`, `storage()`, `max_size() -> NonZeroU64`, `max_pixels() -> NonZeroU64`, and `try_acquire_slot() -> Option<OwnedSemaphorePermit>`.
- Upload staging, JPEG inspection, pixel-count validation, filename parsing, and storage cleanup become crate-visible (`pub(crate)`) `UploadState` methods for the route module; they are not exported outside the API crate. Actix route handlers remain free functions.
- Renames `ApiState::store()` to `ApiState::auth()`; it continues to return `AuthRepository`.

- [ ] **Step 1: Write or adapt tests for the intended upload names**

Update configuration and state tests to construct `UploadConfig` from `NonZeroU64` values, verify the existing 64 MiB / 100,000,000 pixel / four-slot defaults and overrides, and assert that `UploadState::install` initializes resources and `try_acquire_slot` refuses excess slots but reacquires after a permit is dropped. Keep route regressions for truncated SOS rejection and idle-body timeout cleanup.

- [ ] **Step 2: Run the targeted tests and confirm they fail against the old API**

Run: `cargo test -p pp-api --locked --lib photo_upload`
Expected: FAIL to compile after tests are written against the renamed config type and fields.

- [ ] **Step 3: Rename the upload module and remove redundant numeric wrappers**

Move the module to `upload.rs`, rename `PhotoUploadConfig` to `UploadConfig` and `PhotoUploadState` to `UploadState`, and use `NonZeroU64` directly for size and pixel limits. Preserve configuration field, TOML section, and environment variable spellings.

- [ ] **Step 4: Move upload helpers onto `UploadState` and update callers**

Implement `UploadState::install` to initialize libvips and construct shared resources. Convert state-dependent helper functions to private methods, rename `try_acquire_upload_slot` to `try_acquire_slot`, rename `ApiState::store` to `auth`, and update route and API wiring.

- [ ] **Step 5: Run upload configuration, state, and route regressions**

Run: `cargo test -p pp-api --locked --lib photo_upload && cargo test -p pp-api --locked --lib upload::tests && cargo test -p pp-api --locked --lib routes::photos::tests`
Expected: PASS, including config defaults/overrides, upload slot and staged-file cleanup, and route-level timeout/JPEG validation.

- [ ] **Step 6: Commit the upload API refactor**

```bash
git add crates/api
git commit -m "refactor: simplify photo upload API types"
```

### Task 2: Carry failure reasons in photo status

**Files:**
- Modify: `crates/database/migrations/0006_photos.sql`
- Modify: `crates/database/src/photo.rs`
- Modify: `crates/api/src/routes/photos.rs`
- Test: `crates/database/src/photo.rs`
- Test: `crates/api/src/routes/photos.rs`

**Interfaces:**
- Produces `PhotoStatus::{Waiting, Processing, Ready, Failed(String)}`; it is no longer `Copy` or directly derived as a PostgreSQL enum.
- `Photo` stores only `status`, not a separate `failure_reason` field. Failed JSON serializes as `{"status":{"failed":"<reason>"}}`; other status variants remain strings.
- PostgreSQL continues storing its `photo_status` enum and `failure_reason` column, with a check constraint requiring the reason exactly when status is `failed`. A private SQLx enum decodes the PostgreSQL status before it is combined with the nullable reason.
- `PhotoSize` is replaced by `NonZeroU64`; `PhotoDimensions` remains.

- [ ] **Step 1: Write failing tests for the status representation and database invariant**

Update `listing_returns_all_states_in_creation_and_id_order` to assert `"status": {"failed": "processing failed"}` and no top-level `failure_reason`, while other states remain strings. Add migration-backed tests that reject `failed` without a reason and a non-failed state with a reason.

- [ ] **Step 2: Run the targeted tests and confirm the old model fails**

Run: `cargo test -p pp-database --locked photo::tests && cargo test -p pp-api --locked --lib listing_returns_all_states_in_creation_and_id_order`
Expected: FAIL because the current model uses a unit `Failed` variant and exposes a separate reason field.

- [ ] **Step 3: Implement the status model and storage invariant**

Add the migration check constraint, replace `PhotoSize` with `NonZeroU64`, and decode the PostgreSQL status and optional reason together. Reject inconsistent database values as decode errors rather than fabricating a status.

- [ ] **Step 4: Update API serialization assertions**

Remove `failure_reason` from the serialized `Photo` shape and assert the externally tagged failed variant while preserving string serialization for waiting, processing, and ready.

- [ ] **Step 5: Run database and photo-list tests**

Run: `cargo test -p pp-database --locked photo::tests && cargo test -p pp-api --locked --lib routes::photos::tests::listing`
Expected: PASS, including both database constraint regressions and all four serialized states.

- [ ] **Step 6: Commit the photo status refactor**

```bash
git add crates/database crates/api/src/routes/photos.rs
git commit -m "refactor: carry failure reason in photo status"
```

### Task 3: Coordinate photo acceptance in `PhotoIntake`

**Files:**
- Create: `crates/database/src/photo_intake.rs`
- Modify: `crates/database/src/lib.rs`
- Modify: `crates/database/src/jobs.rs`
- Modify: `crates/database/src/photo.rs`
- Modify: `crates/api/src/api.rs`
- Modify: `crates/api/src/routes/photos.rs`
- Test: `crates/database/src/photo_intake.rs`
- Test: `crates/api/src/routes/photos.rs`

**Interfaces:**
- Produces `PhotoRepository::insert_in(&mut Transaction<'_, Postgres>, &NewPhoto) -> Result<PhotoInsertOutcome, DatabaseError>`; this is the photo repository's only insert operation. `PhotoInsertOutcome` distinguishes a newly inserted row, an identical existing row (for idempotent reconciliation), and a missing/foreign gallery. An ID collision with different photo data is an error.
- Produces `PhotoIntake<'db>` implementing `Repository<'db>` and `PhotoIntake::accept(NewPhoto) -> Result<Option<Photo>, PhotoIntakeError>`. It creates one transaction, calls `PhotoRepository::insert_in` and `JobRepository::enqueue_in`, and commits both together.
- Moves and renames `PhotoCreateError` to `PhotoIntakeError`, retaining `Database(DatabaseError)` and `CommitOutcomeUnknown { commit_error, reconciliation_error }`; document its cleanup contract with a runnable example.
- `PhotoIntake` owns idempotent reconciliation after an uncertain commit; unresolved outcomes remain distinguishable so the API preserves the original. Reconciliation enqueues only when it inserts a row, never when the identical photo already exists.
- The route calls `ApiState::photo_intake()` rather than asking `PhotoRepository` to enqueue jobs.
- Keep `JobRepository::enqueue` and `enqueue_in`; standalone job creation and caller-owned transaction usage both have existing callers.

- [ ] **Step 1: Write failing tests for photo-only insertion and `PhotoIntake` coordination**

Add tests that `PhotoRepository::insert_in` writes only photo metadata, and that `PhotoIntake::accept` atomically writes photo/job rows, returns `None` for a missing or foreign gallery, and rolls back both rows if job insertion fails. Exercise reconciliation after an injected commit-classification error against an already-accepted photo; assert it returns the same photo and leaves one job. Preserve the existing unit coverage for unresolved commit classification and add a storage cleanup test proving an unknown outcome preserves the published original.

Add a route regression with a gated request-body stream: wait until the route polls the body (after its gallery check), delete the gallery, then release a valid JPEG body. Assert a 404 response, no photo/job rows, and removal of the published original.

- [ ] **Step 2: Run the database and route tests to confirm the new coordinator is missing**

Run: `cargo test -p pp-database --locked photo::tests && cargo test -p pp-api --locked --lib routes::photos::tests`
Expected: FAIL because acceptance still lives in `PhotoRepository` and the route has no `PhotoIntake` path.

- [ ] **Step 3: Separate photo insertion from job enqueue**

Implement the transaction-taking `PhotoRepository::insert_in` and `PhotoInsertOutcome`. Insert with conflict handling for the generated photo ID; only treat a conflict as idempotent when the stored row matches the requested photo. Do not enqueue a job from this method, add a standalone insert variant, or add a generic database-or-transaction executor abstraction.

- [ ] **Step 4: Implement `PhotoIntake::accept`**

Move transaction creation, job enqueue, commit classification, and idempotent commit-outcome reconciliation out of `PhotoRepository` into `PhotoIntake`. Reuse `JobRepository::enqueue_in`.
Fix the review's missing blank line in `crates/database/src/jobs.rs` without changing job behavior.

- [ ] **Step 5: Wire the upload route to the coordinator**

Expose `PhotoIntake` through `ApiState`, use it after publishing the original, and preserve cleanup behavior: remove originals only after known failures; preserve them when commit outcome remains unknown.

- [ ] **Step 6: Run targeted database and route tests**

Run: `cargo test -p pp-database --locked photo && cargo test -p pp-api --locked --lib routes::photos::tests`
Expected: PASS, including atomicity, reconciliation, cleanup, and route scope cases.

- [ ] **Step 7: Commit the transaction coordinator refactor**

```bash
git add crates/database crates/api
git commit -m "refactor: coordinate photo acceptance transaction"
```

### Final verification

- [ ] Run `cargo +nightly fmt --all --check`.
- [ ] Run `cargo test --workspace --locked`.
- [ ] Run `cargo clippy -p pp-api -p pp-database --all-targets --locked -- -D warnings`.
- [ ] Review the final diff against the approved spec and reply to each agreed PR review thread; resolve only threads whose requested change is implemented and verified.
