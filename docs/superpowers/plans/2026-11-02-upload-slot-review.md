# Upload Slot Review Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Address the second review of PR #132 with typed upload admission, simpler shared state,
and focused persistence API cleanup.

**Architecture:** An eager `UploadSlot` extractor owns the semaphore permit and exposes staging.
`ApiState` owns upload resources and the database; routes choose repositories through `database()`.
Database storage and atomic photo/job acceptance remain unchanged.

**Tech Stack:** Rust, Actix Web, Tokio, SQLx/PostgreSQL, libvips.

**Spec:** `docs/superpowers/specs/2026-10-31-photo-upload-design.md`, approved second PR review refinement.

**Issue:** #98. **PR:** #132. Execute inline in the existing isolated worktree.

## Global Constraints

- Keep the module named `upload`.
- Do not consume a body before authorization.
- A saturated server can return `503` before gallery/header validation; this tradeoff is accepted.
- Keep the PostgreSQL enum and constraint.
- Keep the retry bounded and preserve atomicity and reconciliation.
- Do not change unrelated repository APIs or add dependencies.
- Prefix new GitHub review-thread replies with `[agent]`.

## Review Focus

- A validation failure or cancelled request releases its acquired slot without leaking staged files.
- A pending upload holds its slot; another request fails fast with `503`, even with invalid headers.
- Authentication failure does not consume the body or leak a concurrently extracted slot.
- One Rust status enum still rejects unknown states and inconsistent status/reason combinations.
- Fatal/panic or unavailable commit severity remains uncertain and preserves a possibly committed original.

---

### Task 1: Eager upload slots and unified application state

**Files:**
- Modify: `crates/api/src/upload.rs`
- Modify: `crates/api/src/api.rs`
- Modify: `crates/api/src/routes/photos.rs`
- Modify: `crates/api/src/tests.rs`
- Modify: existing API tests and callers affected by state construction

**Interfaces:**
- `upload::install() -> Result<(), PhotoUploadError>` initializes process-wide libvips resources.
- `UploadState::new(storage: PhotoStorage, config: UploadConfig) -> Self` constructs bounded resources.
- `ApiState::new` additionally receives `UploadState`; `ApiState::upload() -> &UploadState`.
- `ApiState::database() -> &Database` replaces repository-specific getters everywhere.
- `UploadSlot` implements `FromRequest<Error = ErrorResponse>` and owns `Data<ApiState>` plus an
  `OwnedSemaphorePermit`; no public constructor can bypass permit acquisition.
- `UploadSlot::stage_payload(&self, Payload) -> Result<(StagedPhoto, NonZeroU64), ErrorResponse>`.
- Keep resource-dependent image inspection/publication methods on `UploadState`; rename cleanup
  to `cleanup_after_failure`.

- [ ] **Step 1: Add failing admission and lifetime tests**

  Add HTTP tests with concurrency set to one. Gate a valid upload body after authorization:
  another upload receives `503`; cancelling the gated request allows a later valid upload to
  receive `201`, with no staged-file leak. With a held slot, an invalid content type receives
  `503`; after release it receives `415`. Repeat an invalid-header request followed by a valid
  request to prove validation releases the slot. An unauthenticated request must not poll its
  body, and must not prevent a subsequent authenticated upload.

  Adapt existing staging/timeout tests to stage through a real acquired `UploadSlot`. Preserve
  existing configured limits, JPEG regressions, and the gallery-deletion race.

- [ ] **Step 2: Run the tests before implementation**

  Run `cargo test -p pp-api --locked --lib`. Expect missing slot/state APIs and/or the old
  validation-before-admission behavior to fail the new expectations. Record the actual RED.

- [ ] **Step 3: Implement typed admission and shared-state wiring**

  Register only `Data<ApiState>`. In the extractor, clone that state and acquire its semaphore
  non-blockingly; map exhaustion to `503`. The extractor must not read the body. Keep the
  permit bound for the full handler lifetime, including database acceptance and cleanup.
  Move staging and its idle-timeout helper onto `UploadSlot`; expose shared resources through
  the state rather than copying configuration or introducing another shared-state wrapper.

  Expose the free initializer and update startup. Update every repository getter call, including
  authentication extractors and tests, to the existing `Database::repository` API. Update test
  setup to supply real upload resources; temporary directories must remain owned for each test
  and be cleaned up on drop, not persisted with `TempDir::keep`.

  Explain the route's `Ok(None)` cleanup branch and rename its failure-cleanup call.

- [ ] **Step 4: Verify API behavior**

  Run `cargo test -p pp-api --locked --lib`. Expect all new admission/lifetime tests and existing
  API tests to pass, including authentication, header validation, timeout cleanup, JPEG validation,
  and gallery ownership checks.

- [ ] **Step 5: Commit**

  Commit the coherent API change as `refactor: encode upload admission in request slots`,
  including the required co-author trailer.

### Task 2: One status enum and clearer intake naming

**Files:**
- Modify: `crates/database/src/photo.rs`
- Modify: `crates/database/src/photo_intake.rs`
- Modify: `crates/api/src/routes/photos.rs`
- Modify: directly affected documentation and tests

**Interfaces:**
- Keep `PhotoStatus::{Waiting, Processing, Ready, Failed(String)}`; remove `DatabasePhotoStatus`.
- Decode `status::text AS status` at every photo-row SQL boundary; row decoding reads a string
  and optional reason and rejects invalid combinations without a fallback.
- `PhotoIntake::try_accept(NewPhoto) -> Result<Option<Photo>, PhotoIntakeError>` is the public,
  bounded-retry entry point; private `accept(&NewPhoto)` performs one transaction.
- `PhotoIntakeError::from_commit(sqlx::Error) -> Self` replaces `from_commit_error`.
- Private `PgSeverity` extension trait exposes `commit_outcome_is_unknown(self) -> bool`.
  Unavailable severity is separately treated as unknown; FATAL and PANIC remain unknown.

- [ ] **Step 1: Add or adapt focused failing tests**

  Exercise decoding with failed/missing reason, non-failed/present reason, and unknown status;
  assert errors, not fabricated states. Retain all four valid-state serialization assertions and
  both database-constraint regressions. Adapt intake tests to `try_accept` and the severity
  extension method, preserving expectations for ordinary ERROR, FATAL, PANIC, and unavailable
  severity.

- [ ] **Step 2: Confirm RED**

  Run `cargo test -p pp-database --locked photo`. Expect the changed APIs to be missing before
  implementation; confirm the failing cases are relevant rather than environment failures.

- [ ] **Step 3: Implement database-boundary and naming changes**

  Remove the second Rust enum and SQLx enum derive; cast status to text in INSERT RETURNING,
  collision reconciliation SELECT, and listing SELECT. Keep the SQL schema unchanged.
  Rename and update all intake call sites and docs; remove any redundant reconciliation
  forwarding method if it only calls the renamed single-attempt operation. Put severity policy
  on the private extension trait without changing the conservative unknown-outcome behavior.
  Add the requested blank lines before control-flow blocks.

- [ ] **Step 4: Verify persistence and route contracts**

  Run `cargo test -p pp-database --locked photo` and
  `cargo test -p pp-api --locked --lib routes::photos::tests`.
  Expect status, atomicity, collision, rollback, reconciliation, and cleanup tests to pass.

- [ ] **Step 5: Commit**

  Commit as `refactor: simplify photo status decoding and intake naming`,
  including the required co-author trailer.

### Final verification and delivery

- [ ] Update related permanent documentation to match the approved refinement.
- [ ] Run `cargo +nightly fmt --all --check`.
- [ ] Run `cargo test --workspace --locked`.
- [ ] Run `cargo clippy -p pp-api -p pp-database --all-targets --locked -- -D warnings`.
- [ ] Request one fresh independent review against the approved refinement and Review Focus.
- [ ] Address material findings and verify corrections.
- [ ] Push to the existing PR branch and reply to every new review thread with `[agent]`.
- [ ] Preserve the worktree and plan through review corrections; do not merge.

Use the established local test environment when needed:
`DATABASE_URL=postgres://postgres@127.0.0.1:5432/photo_proofing_test`,
`LIBRARY_PATH=/tmp/photo-proofing-libvips/root/usr/lib/x86_64-linux-gnu`,
`LD_LIBRARY_PATH=/tmp/photo-proofing-libvips/root/usr/lib/x86_64-linux-gnu`.
