# Photo Upload Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a photographer upload individual JPEGs to a gallery, preserve accepted originals unchanged, enqueue processing durably, and view every photo's processing state.

**Architecture:** Add a database photo model and workspace-scoped repository, with photo creation and its tagged `process_photo` job committed atomically. Stream each request into `PhotoStorage`'s staging file, inspect the completed staged file's JPEG header before publishing the original, then expose a workspace-scoped list route.

**Tech Stack:** Rust 2024, Actix Web, SQLx/PostgreSQL, Tokio, `pp-storage`, libvips for metadata-only JPEG header inspection.

**Spec:** `docs/superpowers/specs/2026-10-31-photo-upload-design.md` (approved; issue #98)

## Global Constraints

- One raw `image/jpeg` request body contains one file; parallel per-file requests form a batch.
- Default limits are 64 MiB per file, 100,000,000 pixels, and four concurrent uploads; each is configurable.
- Reject a declared over-limit `Content-Length` before reading and enforce the byte limit while streaming.
- Check JPEG content and dimensions from the header without decoding pixels; return `415` for wrong media type, `413` for excessive size, and `422` for malformed JPEG or excessive dimensions.
- Store accepted original bytes unchanged; filenames are display metadata only and never form paths.
- The stored original, photo row, and `process_photo` job must not be left partially accepted after an ordinary request failure; photo and job rows commit in one transaction.
- The photo list includes every state (`waiting`, `processing`, `ready`, `failed`) and is ordered oldest-first with a stable tie-break.
- This slice does not implement processing, derivatives, client delivery, upload UI, or live updates.

## Implementation Calls

- Use a 255-byte UTF-8 display-filename cap; invalid, NUL-containing, or overlong values use `photo.jpg` rather than rejecting the upload.
- Refuse uploads beyond the four active slots with `503`, rather than retaining additional open requests.
- Order photos oldest-first, with photo ID as the tie-break for identical creation timestamps.
- Inspect only the random storage-owned staging path, never a request filename; return explicit cleanup failures rather than silently ignoring them.
- Treat a disconnected client as a terminated request that still cleans up its stage; map storage-full to `503`.

## Review Focus

- A chunked request with no `Content-Length` exceeds the byte limit: return `413` and discard the staged file.
- A truncated or malformed JPEG header: return `422`, leaving no original, photo row, or job.
- Dimensions overflow during pixel-count calculation or exceed the configured pixel limit: return `422` before decoding.
- A disconnected client or full photo volume: discard staged content and leave no photo or job; storage-full returns `503`, while a disconnected client cannot receive a response.
- A foreign-workspace gallery is indistinguishable from a missing gallery, while duplicate filenames in one gallery remain valid separate uploads.

---

### Task 1: Persist photos and enqueue processing atomically

**Files:**
- Create: `crates/database/migrations/0006_photos.sql`
- Create: `crates/database/src/photo.rs`
- Modify: `crates/database/src/lib.rs`
- Modify: `crates/database/src/jobs.rs`
- Modify: `crates/core/src/notifications.rs`
- Test: `crates/database/src/photo.rs`
- Test: `crates/database/src/jobs.rs`

**Interfaces:**
- Uses the existing `pp_core::notifications::PhotoId`, with serde and feature-gated SQLx encodings added for database and JSON boundaries.
- Produces `PhotoStatus` (`Waiting`, `Processing`, `Ready`, `Failed`), `PhotoSize`, `PhotoDimensions`, `Photo`, and `NewPhoto` in `pp_database::photo`.
- Produces `PhotoRepository::create_with_job(NewPhoto) -> Result<Option<Photo>, DatabaseError>` (`None` when the gallery is missing or belongs to another workspace) and `PhotoRepository::list(WorkspaceId, GalleryId) -> Result<Vec<Photo>, DatabaseError>`.
- Produces `JobRepository::enqueue_in(&mut Transaction<'_, Postgres>, JobPayload, NonZeroU16) -> Result<JobId, DatabaseError>`.
- The processing payload is tagged `process_photo` with `{"photo_id":"<uuid>"}` as its body.

- [ ] **Step 1: Write failing migration-backed repository tests**

Add tests that create a photo with its job, list photos in creation order across all four statuses, keep duplicate names distinct, and verify another workspace cannot list or create a photo in the gallery (`create_with_job` returns `None`). Assert the job payload has the `process_photo` discriminator and the photo UUID.

- [ ] **Step 2: Run the targeted tests and verify the missing schema/repository fails**

Run: `cargo test -p pp-database --locked photo::tests`
Expected: FAIL because the photo migration and repository do not exist yet.

- [ ] **Step 3: Add the photo schema and typed repository**

Create `photo_status` and `photos` with workspace and gallery ownership, display filename, original size, header dimensions, status, failure reason, and creation time. Add the scoped gallery index and use the repository contract above; preserve typed units in Rust instead of unit-suffixed primitive fields.

- [ ] **Step 4: Add transaction-aware job enqueue and atomic photo creation**

Implement `enqueue_in` using the existing tagged `JobPayload` serialization. `create_with_job` inserts the photo and job in one SQL transaction and returns the inserted photo only after commit; constrain the gallery by both workspace and gallery ID, returning `None` when that scoped gallery does not exist.

- [ ] **Step 5: Run database tests and verify they pass**

Run: `cargo test -p pp-database --locked photo::tests`
Expected: PASS, including atomicity, cross-workspace scope, ordering, and duplicate-name coverage.

- [ ] **Step 6: Commit the database deliverable**

```bash
git add crates/core/src/notifications.rs crates/database
git commit -m "feat: persist uploaded photos and processing jobs"
```

### Task 2: Configure bounded upload resources

**Files:**
- Modify: `crates/api/src/config.rs`
- Modify: `crates/api/src/api.rs`
- Modify: `crates/api/src/lib.rs`
- Create: `crates/api/src/photo_upload.rs`
- Modify: `Cargo.toml`
- Modify: `crates/api/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/storage/src/lib.rs`
- Modify: `README.md`
- Modify: `config.example.toml`
- Modify: `docker/api.Dockerfile`
- Modify: `.github/workflows/ci.yml`
- Test: `crates/api/src/config.rs`
- Test: `crates/api/src/photo_upload.rs`
- Test: `crates/storage/src/lib.rs`

**Interfaces:**
- Produces typed upload limits for maximum file size, maximum pixel count, and concurrent upload slots, with defaults of 64 MiB, 100,000,000 pixels, and four slots.
- Produces `PhotoUploadState` in `crates/api/src/photo_upload.rs`, holding the already-open `PhotoStorage`, validated limits, and a semaphore; excess concurrent uploads fail fast with `503`.
- `Api::init` opens and retains the storage handle and initializes the upload state once at startup.
- `StagedPhoto::temporary_path()` exposes only the storage-generated staging path for the metadata loader; it does not accept or derive from a filename.

- [ ] **Step 1: Write failing configuration tests**

Test the three defaults, explicit config overrides, rejection of zero concurrency, acquisition/refusal of the fourth/fifth concurrent upload slot, and access to a staged file's temporary path.

- [ ] **Step 2: Run the targeted tests and verify they fail**

Run: `cargo test -p pp-api --locked photo_upload`
Expected: FAIL because upload settings and state do not exist yet.

- [ ] **Step 3: Implement typed limits and startup state**

Add unit-carrying size and pixel-limit types, a nonzero concurrency setting, and `PhotoUploadState`. Keep the storage handle in the upload state rather than reopening the configured directory per request.

- [ ] **Step 4: Wire libvips metadata support into the API runtime**

Add `StagedPhoto::temporary_path()` with a contract that the path is random and storage-owned, and test that it points to the staged file but not a published photo key. Use the approved design's libvips family to inspect JPEG headers without decoding pixels. Initialize it once; add the required API build/runtime packages to `docker/api.Dockerfile` and CI so local, container, and CI builds use the same dependency.

- [ ] **Step 5: Run API config tests and verify they pass**

Run: `cargo test -p pp-api --locked photo_upload`
Expected: PASS for defaults, overrides, and concurrency bounds.

- [ ] **Step 6: Commit the upload-resource deliverable**

```bash
git add Cargo.toml Cargo.lock crates/api crates/storage README.md config.example.toml docker/api.Dockerfile .github/workflows/ci.yml
git commit -m "feat: configure bounded photo uploads"
```

### Task 3: Stream and validate individual photo uploads

**Files:**
- Create: `crates/api/src/routes/photos.rs`
- Modify: `crates/api/src/routes.rs`
- Modify: `crates/api/src/error.rs`
- Modify: `crates/api/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/api/src/routes/photos.rs`

**Interfaces:**
- Registers `POST /api/v1/galleries/{id}/photos`, taking an authenticated workspace identity, gallery ID, `Content-Type: image/jpeg`, percent-encoded `X-Filename`, and a streamed request body. It verifies workspace ownership before reading the body and rechecks it in the photo/job transaction.
- Returns `201` with the created photo; maps invalid media type, excessive size, malformed/oversized dimensions, storage-full, and unavailable capacity to explicit client errors.
- Reuses `PhotoUploadState` for bounded storage, settings, and upload concurrency.

- [ ] **Step 1: Write failing route and upload-pipeline tests**

Cover valid JPEG preservation and job creation, wrong media type, declared and streamed oversize bodies, truncated/corrupt JPEG, excessive and overflowing dimensions, malformed filename fallback, duplicate filenames, cross-workspace denial, interrupted body, and storage-full cleanup. Assert each rejected request leaves no published original, photo row, or job.

- [ ] **Step 2: Run the targeted tests and verify they fail**

Run: `cargo test -p pp-api --locked routes::photos::tests`
Expected: FAIL because the upload route and validation path do not exist.

- [ ] **Step 3: Implement streamed validation and staged storage**

Reject missing or foreign galleries, wrong content type, excessive declared length, and unavailable concurrency before body processing. Stream chunks into a `StagedPhoto`, enforce the configured byte count for unknown-length bodies, flush it, inspect JPEG format and dimensions through the storage-owned temporary path with checked pixel multiplication, and map each rejection to the specified status.

- [ ] **Step 4: Publish the original and persist its photo/job pair**

Generate a `PhotoId`, decode and bound the display filename (fall back to `photo.jpg` for invalid, NUL-containing, or overlong values), atomically commit the unchanged staged original under its UUID key, then call `PhotoRepository::create_with_job`. If database creation fails, remove the just-published original and report any cleanup failure; dropping an uncommitted stage handles body and validation failures.

- [ ] **Step 5: Run the targeted route tests and verify they pass**

Run: `cargo test -p pp-api --locked routes::photos::tests`
Expected: PASS, including cleanup and no partial photo/job records on rejection.

- [ ] **Step 6: Commit the upload route**

```bash
git add crates/api Cargo.lock
git commit -m "feat: accept validated JPEG photo uploads"
```

### Task 4: List gallery photos and verify the full API slice

**Files:**
- Modify: `crates/api/src/routes/photos.rs`
- Modify: `crates/api/src/routes.rs`
- Test: `crates/api/src/routes/photos.rs`

**Interfaces:**
- Registers `GET /api/v1/galleries/{id}/photos` behind `Authenticated`.
- Returns all photos owned by the requested workspace and gallery, including `id`, `original_name`, `size`, `width`, `height`, `status`, `failure_reason`, and `created_at`; a missing or foreign-workspace gallery returns the existing 404 response.

- [ ] **Step 1: Write failing list-route tests**

Test an empty gallery, a gallery with all four statuses, oldest-first ordering with a stable tie-break, unauthorized access, and cross-workspace 404 behavior.

- [ ] **Step 2: Run the targeted list tests and verify they fail**

Run: `cargo test -p pp-api --locked routes::photos::tests::listing`
Expected: FAIL because the list route is not registered.

- [ ] **Step 3: Implement the workspace-scoped list route**

Resolve the gallery through the authenticated workspace before listing its photos. Return `no-store` JSON using the existing response extension and repository order.

- [ ] **Step 4: Run API checks**

Run: `cargo test -p pp-api --locked && cargo clippy -p pp-api -p pp-database --all-targets --locked -- -D warnings`
Expected: PASS with all upload, listing, database, and existing API tests green.

- [ ] **Step 5: Commit the list route and regression tests**

```bash
git add crates/api
git commit -m "feat: list gallery photo processing states"
```
