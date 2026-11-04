# Proof thumbnails and previews Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The image worker turns each uploaded original into a thumbnail and a preview, keeps `photos.status` truthful, supports manual retry, and the API serves ready derivatives to the photographer.

**Architecture:** A typed `ProcessPhoto` job payload feeds a worker handler that renders with libvips and publishes through `PhotoStorage`. `PhotoRepository` owns every status transition; the job repository calls it whenever a job becomes `failed`, so a photo is never stuck on `processing`. Retry and serving are new API routes.

**Tech Stack:** Rust, SQLx/PostgreSQL, libvips (`libvips` crate 2.3), actix-web, tokio.

**Spec:** `docs/superpowers/specs/2026-11-04-proof-derivatives-design.md` (issue #99). Rust rules: `crates/AGENTS.md`.

## Global Constraints

- Thumbnail 400 px and preview 1600 px on the long edge, JPEG quality about 75, never upscaled; constants, no settings UI.
- Derivatives: auto-rotated, converted to sRGB with no embedded profile, all metadata stripped (EXIF, XMP, IPTC, GPS).
- Originals are never modified; derivatives use keys `Variant::Thumbnail` / `Variant::Preview` and replace on rerun (`StagedPhoto::commit` already does this).
- `photos.status` is the source of truth; `failure_reason` is always a short fixed safe message, never raw error text.
- Stored job JSON stays `{"kind": "process_photo", "payload": {"photo_id": "…"}}`.
- Missing and foreign galleries/photos both return 404; no route serves originals.
- Blocking libvips work runs through `spawn_blocking`; multi-step state changes run in one transaction (`crates/AGENTS.md`).
- Checks before delivery: the `crates/` format, lint and test commands in `crates/AGENTS.md`.

## Review Focus

- A photo whose job crash-loops (lease expires on the final attempt) ends `failed`, not `processing` (Task 3).
- A stale job failing after the photo became `ready`, or after a manual retry, leaves the photo untouched (Tasks 2, 3).
- Retrying a photo that permanently failed (its job was completed, so no failed job exists) still reprocesses it (Task 4).
- Repeated retry calls on the same gallery enqueue one job per failed photo, not more (Task 4).
- A photo deleted while its job runs leaves no derivative files and no resurrected row (Task 6).
- A JPEG truncated mid-stream or with a lying header is permanent `failed`, not endlessly retried (Tasks 5, 6).
- A source far larger than the preview (for example 100 MP) is handled without exceeding the pixel bound (Task 5).

---

### Task 1: Typed `ProcessPhoto` payload

**Files:**
- Modify: `crates/database/src/jobs.rs` (payload enum, `Job::payload`), `crates/database/src/photo_intake.rs:149`, `crates/image-worker/src/runtime.rs` (tests that build `JobPayload::ProcessPhoto(json)`)
- Test: `crates/database/src/jobs.rs` tests module

**Interfaces:**
- Produces: `pub struct ProcessPhoto { pub photo_id: PhotoId }` (serde `Serialize`/`Deserialize`, `Debug, Clone, PartialEq, Eq`) and `JobPayload::ProcessPhoto(ProcessPhoto)`. `Job::payload::<ProcessPhoto>()` returns it.

- [ ] **Step 1: Write the failing test** `process_photo_payload_keeps_the_stored_shape` in `jobs.rs`: serializing `JobPayload::ProcessPhoto(ProcessPhoto { photo_id })` equals `json!({"kind":"process_photo","payload":{"photo_id": photo_id.to_string()}})`, and deserializing that JSON round-trips.
- [ ] **Step 2: Run** `cargo test -p pp-database process_photo_payload` — expected: compile failure (`ProcessPhoto` undefined).
- [ ] **Step 3: Implement** the struct and change the variant; make `JobPayload::body` return the serialized struct (`serde_json::Value`) so `Job::payload::<T>()` keeps working. Update `photo_intake.rs` to construct the typed payload, and update the `runtime.rs` tests (their ad hoc `{"photo": 1}` payloads become `ProcessPhoto { photo_id }` values, the test `Payload` struct becomes `ProcessPhoto`; the unreadable-payload test keeps a raw SQL insert of a malformed body).
- [ ] **Step 4: Run** `cargo test -p pp-database -p pp-image-worker` — expected: PASS, including the existing `payload_migration_preserves_existing_job_data` and intake payload-shape tests.
- [ ] **Step 5: Commit** `refactor(database): type the ProcessPhoto job payload`.

---

### Task 2: Photo status transitions and lookups

**Files:**
- Modify: `crates/database/src/photo.rs`
- Test: same file, `#[sqlx::test(migrations = "./migrations")]` tests

**Interfaces:**
- Produces on `PhotoRepository`:
  - `pub async fn locate(&self, id: PhotoId) -> Result<Option<PhotoLocation>, DatabaseError>` where `pub struct PhotoLocation { pub workspace_id: WorkspaceId, pub gallery_id: GalleryId }`
  - `pub async fn get(&self, workspace_id: WorkspaceId, gallery_id: GalleryId, id: PhotoId) -> Result<Option<Photo>, DatabaseError>`
  - `pub async fn mark_processing(&self, id: PhotoId) -> Result<bool, DatabaseError>`
  - `pub async fn mark_ready(&self, id: PhotoId) -> Result<bool, DatabaseError>`
  - `pub async fn mark_failed(&self, id: PhotoId, reason: &str) -> Result<bool, DatabaseError>`
  - `pub async fn mark_failed_in(transaction: &mut Transaction<'_, Postgres>, id: PhotoId, reason: &str) -> Result<bool, DatabaseError>` (associated function, used by Task 3)
  - `pub const PROCESSING_FAILED_REASON: &str` (for example "Processing failed after several attempts.")
  - Each `mark_*` returns `true` only when a row changed. `mark_processing`/`mark_ready`/`mark_failed` act only on photos currently `waiting` or `processing`.

- [ ] **Step 1: Write failing tests** (insert a photo through `PhotoIntake`): `mark_ready_requires_a_waiting_or_processing_photo` (ready → `mark_ready` false and `mark_failed` false, status unchanged); `mark_failed_stores_the_reason_and_listing_shows_it`; `mark_processing_is_repeatable_after_a_crash` (two calls both true); `get_is_scoped_to_workspace_and_gallery` (foreign workspace or gallery → `None`); `locate_returns_none_for_a_missing_photo`.
- [ ] **Step 2: Run** `cargo test -p pp-database photo::` — expected: compile failure.
- [ ] **Step 3: Implement** the methods with single `UPDATE … WHERE id = $1 AND status IN ('waiting','processing')`; failed sets `failure_reason`, ready/processing clear it (the table CHECK ties `failed` to a reason).
- [ ] **Step 4: Run** the same command — expected: PASS.
- [ ] **Step 5: Commit** `feat(database): photo processing transitions`.

---

### Task 3: Job failure marks the photo failed

**Files:**
- Modify: `crates/database/src/jobs.rs` (`fail`, `claim`)
- Test: `crates/database/src/jobs.rs` tests (need photos: use `PhotoIntake` or direct inserts)

**Interfaces:**
- Consumes: `PhotoRepository::mark_failed_in`, `PROCESSING_FAILED_REASON` (Task 2), typed payload (Task 1).
- Produces: unchanged signatures; `JobRepository::fail` and `claim` now also fail the owning photo, in the same transaction, whenever a job becomes `failed`. `abandon` (unreadable payload) is unchanged: it has no photo id.

- [ ] **Step 1: Write failing tests:** `exhausted_job_fails_its_photo` (enqueue via intake with `max_attempts` 1, claim, `fail` → job `failed`, photo `Failed(PROCESSING_FAILED_REASON)`); `retryable_failure_leaves_the_photo_alone` (attempts remain → photo still waiting/processing); `crash_looping_job_fails_its_photo` (claim until lease expires on the final attempt, next `claim` marks job failed and photo failed); `late_failure_does_not_touch_a_ready_photo` (mark photo ready first, then exhaust the job → photo stays ready).
- [ ] **Step 2: Run** `cargo test -p pp-database jobs::` — expected: the new tests FAIL (photo not updated).
- [ ] **Step 3: Implement:** in `fail`, run the job update in a transaction using `RETURNING state, payload`; when the state is `failed`, call `mark_failed_in` for the payload's `photo_id`, then commit. In `claim`, change the exhausted-reclaim statement to `RETURNING payload`, mark each returned photo failed in the same transaction. Keep `last_error` as the detailed job error; the photo gets only the fixed reason.
- [ ] **Step 4: Run** `cargo test -p pp-database` — expected: PASS (existing backoff/crash-loop tests still pass).
- [ ] **Step 5: Commit** `feat(database): fail the photo when its job fails`.

---

### Task 4: Manual retry in the database

**Files:**
- Create: `crates/database/src/photo_retry.rs`
- Modify: `crates/database/src/lib.rs` (module), `crates/database/src/jobs.rs` (a transaction-taking delete of failed jobs for photos, if needed)
- Test: `crates/database/src/photo_retry.rs`

**Interfaces:**
- Consumes: `JobRepository::enqueue_in`, `DEFAULT_MAX_ATTEMPTS`, `ProcessPhoto` (Task 1).
- Produces: `pub struct PhotoRetry<'db>` implementing `Repository` (as `PhotoIntake` does), with
  `pub async fn retry_failed(&self, workspace_id: WorkspaceId, gallery_id: GalleryId, photo: Option<PhotoId>) -> Result<Option<u64>, DatabaseError>`: `None` when the gallery does not exist in the workspace; otherwise the number of photos requeued (zero when nothing was failed, including an unknown or non-failed `photo`).

- [ ] **Step 1: Write failing tests:** `retry_requeues_every_failed_photo_in_the_gallery`; `retry_one_photo_leaves_other_failed_photos_failed`; `retry_without_a_failed_job_still_enqueues_one` (photo `failed` via `mark_failed`, no job row); `retry_replaces_an_existing_failed_job` (exactly one job per photo afterwards, none `failed`); `retry_ignores_waiting_processing_and_ready_photos`; `repeating_retry_does_not_duplicate_jobs`; `retry_in_a_foreign_workspace_or_gallery_returns_none`.
- [ ] **Step 2: Run** `cargo test -p pp-database photo_retry` — expected: compile failure.
- [ ] **Step 3: Implement** in one transaction: confirm gallery ownership; `UPDATE photos SET status='waiting', failure_reason=NULL WHERE workspace_id=$1 AND gallery_id=$2 AND status='failed' [AND id=$3] RETURNING id`; delete `jobs` rows in state `failed` whose `payload->'payload'->>'photo_id'` is one of those ids; enqueue a fresh `ProcessPhoto` job per id.
- [ ] **Step 4: Run** `cargo test -p pp-database` — expected: PASS.
- [ ] **Step 5: Commit** `feat(database): retry failed photos`.

---

### Task 5: Rendering derivatives with libvips

**Files:**
- Create: `crates/image-worker/src/render.rs`
- Modify: `crates/image-worker/Cargo.toml` (`libvips.workspace = true`), `crates/image-worker/src/lib.rs`, `crates/core/src/config.rs` (shared default constant), `crates/image-worker/src/config.rs` (pixel bound setting)
- Test: `crates/image-worker/src/render.rs` (generate fixtures with `libvips::ops::black` + `jpegsave_buffer`, as `crates/api/src/upload/jpeg.rs` tests do)

**Interfaces:**
- Produces:
  - `pub const DEFAULT_MAX_PIXELS: NonZeroU64` (100,000,000) in `pp-core`'s config module, used by both the API's `UploadConfig` default and the worker.
  - `WorkerConfig.render: RenderConfig { max_pixels: NonZeroU64 }` (`--photo-process-max-pixels`, `PP_PHOTO_PROCESS_MAX_PIXELS`, default `DEFAULT_MAX_PIXELS`); add to `config.example.toml` under `[photo]`-adjacent section.
  - `pub fn install() -> Result<(), RenderError>` initializing libvips once (concurrency 1, caches off, same settings as the API's `install`).
  - `pub struct Rendered { pub thumbnail: Vec<u8>, pub preview: Vec<u8> }`
  - `pub fn render(original: &[u8], max_pixels: NonZeroU64) -> Result<Rendered, RenderError>` (blocking; callers use `spawn_blocking`)
  - `pub enum RenderError { Unreadable, TooLarge, Library(String) }` with `thiserror`; `Unreadable` and `TooLarge` are permanent, `Library` is transient.

- [ ] **Step 1: Write failing tests** in `render.rs`: `output_fits_the_long_edge_and_keeps_aspect_ratio` (300×200 source stays 300×200, no upscaling; 4000×2000 gives 400×200 thumbnail and 1600×800 preview); `exif_orientation_is_applied` (source with orientation 6 comes out with width and height swapped and no orientation tag); `metadata_is_stripped` (a source carrying EXIF with GPS yields outputs with no EXIF/XMP fields); `non_srgb_source_is_converted_to_srgb` (CMYK or profiled source → sRGB, no embedded profile); `corrupt_input_is_unreadable`; `truncated_jpeg_is_unreadable` (cut mid-scan); `oversized_dimensions_are_too_large_without_decoding` (header says more than `max_pixels`).
- [ ] **Step 2: Run** `cargo test -p pp-image-worker render::` — expected: compile failure.
- [ ] **Step 3: Implement** `render` with the libvips `thumbnail_buffer` operation (box of the edge size, size-down only, auto-rotate on, sRGB export profile) then `jpegsave_buffer` with quality about 75 and strip on. Check header dimensions against `max_pixels` first. Use a fail-on-truncated load option so partial files are rejected.
- [ ] **Step 4: Run** `cargo test -p pp-image-worker render::` and `cargo test -p pp-core` — expected: PASS.
- [ ] **Step 5: Commit** `feat(image-worker): render proof derivatives`.

---

### Task 6: `process_photo` handler

**Files:**
- Create: `crates/image-worker/src/process_photo.rs`
- Modify: `crates/image-worker/src/lib.rs`, `crates/image-worker/src/main.rs` (open storage, call `render::install`, register the handler), `crates/image-worker/Cargo.toml` if tokio features are missing
- Test: `crates/image-worker/src/process_photo.rs` (`#[sqlx::test(migrations = "../database/migrations")]` with a temp `PhotoStorage`)

**Interfaces:**
- Consumes: `ProcessPhoto` (1), `PhotoRepository::{locate, mark_processing, mark_ready, mark_failed}` (2), `render` (5), `PhotoStorage::{read, stage}`, `StagedPhoto::commit`.
- Produces: `pub async fn process_photo(context: Arc<ProcessContext>, payload: ProcessPhoto) -> Result<(), HandlerError>`, where `ProcessContext { database: Database, storage: PhotoStorage, max_pixels: NonZeroU64 }`, registered as `HandlerRegistry::register(JobKind::ProcessPhoto, …)`.

Flow: `locate` (missing photo → `Ok`, nothing to do); `mark_processing` (false → `Ok`, photo already finished or failed); read the original into memory; `render` in `spawn_blocking`; commit thumbnail and preview; `mark_ready`. `Unreadable`/`TooLarge` → `mark_failed` with a fixed message ("The image could not be read." / "The image is too large to process.") then `Ok` so the job completes without retries. `Library` or storage errors → `HandlerError::Failed`. If `mark_ready` returns false, remove the derivatives just written (best effort) and return `Ok`.

- [ ] **Step 1: Write failing tests:** `processing_a_photo_publishes_both_derivatives_and_marks_it_ready` (original unchanged byte for byte); `corrupt_original_fails_permanently_with_a_safe_reason` (job handler returns `Ok`, photo `Failed("The image could not be read.")`); `storage_error_is_returned_for_retry` (original file missing → `HandlerError::Failed`, photo stays `processing`); `rerun_replaces_derivatives_without_duplicates` (run twice; exactly three files for the photo); `late_work_for_a_deleted_photo_leaves_nothing` (delete the gallery before `mark_ready`; no derivative files remain, no error); `worker_restart_mid_job_recovers` (drive the real `Worker` with a short lease; a handler that is dropped mid-job is rerun and ends `ready`).
- [ ] **Step 2: Run** `cargo test -p pp-image-worker process_photo` — expected: compile failure.
- [ ] **Step 3: Implement** the handler per the flow above; wire `main.rs` (storage handle reused, `render::install()?` before the worker starts).
- [ ] **Step 4: Run** `cargo test -p pp-image-worker` — expected: PASS.
- [ ] **Step 5: Commit** `feat(image-worker): process uploaded photos`.

---

### Task 7: API retry and serving routes

**Files:**
- Modify: `crates/api/src/routes/galleries/photos.rs`, `crates/api/src/response.rs` (a `private_cache` helper next to `no_store`)
- Test: `crates/api/src/routes/galleries/photos.rs` tests (reuse its `app`, `create_gallery`, `call_request` helpers)

**Interfaces:**
- Consumes: `PhotoRetry::retry_failed` (4), `PhotoRepository::get` (2), `PhotoStorage::read`.
- Produces routes registered in `configure`:
  - `POST /{id}/photos/retry` → `200 {"requeued": <u64>}`
  - `POST /{id}/photos/{photo_id}/retry` → `200 {"requeued": 0|1}`
  - `GET /{id}/photos/{photo_id}/thumbnail` and `/preview` → `200 image/jpeg`, `Cache-Control: private`, `X-Content-Type-Options: nosniff`; 404 unless the photo is in that gallery and workspace and `Ready`.
  - Register the literal `/photos/retry` before `/photos/{photo_id}/…`. No route addresses `Variant::Original`.

- [ ] **Step 1: Write failing tests:** `retry_requeues_failed_photos_and_reports_the_count`; `retry_one_photo_only_touches_that_photo`; `retry_requires_authentication_and_ownership` (401; foreign gallery 404); `ready_photo_serves_thumbnail_and_preview` (headers and body equal the stored file); `serving_returns_404_for_waiting_processing_and_failed_photos`; `serving_returns_404_across_galleries_and_workspaces`; `serving_requires_authentication`; `no_route_serves_the_original` (`/original` and similar paths are 404).
- [ ] **Step 2: Run** `cargo test -p pp-api photos::` — expected: new tests FAIL.
- [ ] **Step 3: Implement** the handlers following `list_photos` (gallery ownership first, `ErrorResponse::not_found()` for misses, storage errors via the existing `From<StorageError>`); read the small derivative into memory and return it.
- [ ] **Step 4: Run** `cargo test -p pp-api` — expected: PASS.
- [ ] **Step 5: Commit** `feat(api): retry failed photos and serve derivatives`.

---

### Task 8: Packaging, docs and delivery checks

**Files:**
- Modify: `docker/worker.Dockerfile` (build stage installs `libvips-dev pkg-config`, runtime stage installs `libvips42`, mirroring `docker/api.Dockerfile`), `README.md` (status paragraph at line 11 and the client-page note at line 204: processing now exists, upload UI, client delivery and SSE still do not; worker setting `PP_PHOTO_PROCESS_MAX_PIXELS`; new routes), `config.example.toml`, `docs/SPECIFICATIONS.md` only if behavior there changed (it should not)

- [ ] **Step 1:** Apply the Dockerfile and documentation edits above.
- [ ] **Step 2: Build the worker image** `docker build -f docker/worker.Dockerfile -t pp-worker-test .` — expected: success; start it with a dummy `PP_DATABASE_URL` only to confirm the binary loads libvips (it should fail on the database, not on a missing library).
- [ ] **Step 3: Run all checks** in `crates/AGENTS.md` (format, clippy with warnings denied, `cargo test --workspace`; PostgreSQL available for `sqlx::test`) — expected: all PASS.
- [ ] **Step 4: Commit** `docs: describe photo processing; build libvips into the worker image`.
