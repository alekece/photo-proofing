# Gallery lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Archive, restore and delete galleries; every file removal (gallery or photo delete) goes through durable cleanup jobs, plus a startup sweep for orphans.

**Architecture:** Deletes commit the row removal and a cleanup job in one transaction. The image worker serves three new job kinds (`cleanup_gallery`, `cleanup_photos`, `sweep_orphans`) through the existing queue. Archive/restore are two small endpoints over the existing `archived_at` column.

**Tech Stack:** Rust (actix-web, sqlx, tokio), PostgreSQL, SvelteKit.

**Spec:** `docs/designs/2026-11-06-gallery-lifecycle-design.md` (issue #73).

## Global Constraints

- No migration; discriminators stay inside the tagged `JobPayload` JSON.
- Everything is scoped to the session's workspace; foreign or unknown gallery is a 404.
- No free functions in non-test Rust (`crates/AGENTS.md`); no new settings; sweep grace is a 24 hour constant.
- Logs carry ids only. Out of scope: #174 (event stream), #175 (expiry prompt on restore). Do not touch `render.rs`, Dockerfiles, `ci.yml`, `README.md` (#142).

## Review Focus

- Delete racing a running render or upload: no photo row, no file left after cleanup.
- Cleanup run twice, or with files already gone: success.
- Photo-delete query must never remove cleanup jobs.
- Sweep must keep files of existing photos and files younger than the grace period.
- Restore must not change `delivered_at`, `shared_at`, `expires_at`.

---

### Task 1: Cleanup job payloads and sweep enqueue (`crates/database/src/jobs.rs`)

**Interfaces:**
- Produces: `JobKind::{CleanupGallery, CleanupPhotos, SweepOrphans}` (`"cleanup_gallery"`, `"cleanup_photos"`, `"sweep_orphans"`); `CleanupGallery { workspace_id: WorkspaceId, gallery_id: GalleryId }`; `CleanupPhotos { workspace_id, gallery_id, photo_ids: Vec<PhotoId> }`; `JobPayload::{CleanupGallery(CleanupGallery), CleanupPhotos(CleanupPhotos), SweepOrphans}`; `JobRepository::enqueue_sweep(&self, max_attempts: NonZeroU16) -> Result<bool, DatabaseError>` (false when a queued or running sweep exists).

- [ ] Tests: payload JSON round trip for each kind (`kind()` matches, tag stored in JSON); `enqueue_sweep` twice enqueues once, and enqueues again after the first one is completed.
- [ ] Implement the above; update `JobKind::as_str`/`FromStr`.
- [ ] Run `cargo test -p pp-database jobs` — PASS. Commit.

### Task 2: Archive, restore, delete, exists (`crates/database/src/gallery.rs`)

**Interfaces:**
- Consumes: `JobRepository::enqueue_in`, `CleanupGallery`.
- Produces: `GalleryRepository::archive(workspace_id, id) -> Result<Option<Gallery>, DatabaseError>` (sets `archived_at` only if null; `updated_at` untouched); `restore(workspace_id, id)` (clears it); `delete(workspace_id, id, jobs: JobOptions) -> Result<bool, DatabaseError>`; `exists(id: GalleryId) -> Result<bool, DatabaseError>`.

- [ ] Tests: archive/restore round trip and idempotence; restore keeps `delivered_at`/`expires_at`; foreign workspace returns `None`/`false`; `find_shared` is `None` when archived and found again after restore; delete removes gallery, photos, favorites and the gallery's `process_photo` jobs and leaves exactly one `cleanup_gallery` job naming workspace and gallery; delete of a foreign gallery changes nothing and enqueues nothing.
- [ ] Implement. `delete` runs in one transaction: delete the gallery's `process_photo` jobs (matching `payload->>'kind'`), delete the gallery row, enqueue the cleanup job, commit; nothing if no row matched.
- [ ] Run `cargo test -p pp-database gallery` — PASS. Commit.

### Task 3: Photo deletes enqueue cleanup (`crates/database/src/photo.rs`)

**Interfaces:**
- Consumes: `CleanupPhotos`.
- Produces: `PhotoRepository::delete(workspace_id, gallery_id, ids, jobs: JobOptions)` now also enqueues `cleanup_photos` jobs (batches of 500 ids) in the same transaction; its job-removal query matches `process_photo` payloads only.

- [ ] Tests: deleting photos leaves a `cleanup_photos` job with exactly the removed ids; ids not removed (foreign, unknown) are not named; an existing cleanup job survives a later photo delete; nothing enqueued when nothing was deleted.
- [ ] Implement; fix existing callers/tests for the new parameter.
- [ ] Run `cargo test -p pp-database` — PASS. Commit.

### Task 4: Storage removal and orphan sweep (`crates/storage/src/lib.rs`)

**Interfaces:**
- Produces: `PhotoStorage::remove_gallery(&self, workspace: WorkspaceId, gallery: GalleryId) -> Result<(), StorageError>` (missing directory is success); `PhotoStorage::sweep_orphans<F, Fut>(&self, grace: Duration, is_stored: F) -> Result<usize, StorageError>` where `F: Fn(PhotoId) -> Fut`, `Fut: Future<Output = Result<bool, E>>`-style check; removes photo files older than `grace` whose photo is not stored, then removes emptied gallery directories; skips `.tmp`, unparseable names, and younger files; returns the number removed.

- [ ] Tests (tempdir): `remove_gallery` removes files and is repeatable; sweep removes an old orphan, keeps an old file whose photo exists, keeps a young orphan, ignores `.tmp` and foreign names, removes an emptied gallery directory but not a non-empty one.
- [ ] Implement with `tokio::fs`; age from file modification time.
- [ ] Run `cargo test -p pp-storage` — PASS. Commit.

### Task 5: Worker handlers (`crates/image-worker/src/handlers/cleanup.rs`, `handlers.rs`, `main.rs`)

**Interfaces:**
- Consumes: Tasks 1, 4.
- Produces: `CleanupHandler::new(database, storage)` with `gallery(CleanupGallery, Job)`, `photos(CleanupPhotos, Job)`, `sweep((), Job)`, each returning `Result<JobCompletion, HandlerError>` and finishing the job; registered in `main.rs` for the three kinds.

- [ ] Tests (sqlx + tempdir): gallery job removes the directory and completes the job; photo job removes all variants of the named photos only; running a job twice succeeds; sweep job removes old orphans (set mtime back) and keeps files of existing photos.
- [ ] Implement; storage failures become `HandlerError::failed` so the queue retries.
- [ ] Run `cargo test -p pp-image-worker` — PASS. Commit.

### Task 6: API (`crates/api`)

**Interfaces:**
- Consumes: Tasks 2, 3.
- Produces: `POST /api/v1/galleries/{id}/archive` and `/restore` (200 + gallery JSON), `DELETE /api/v1/galleries/{id}` (204), each 404 for unknown/foreign; photo delete routes stop removing files; `Api::init` enqueues the sweep after migrating (failure is logged, not fatal).

- [ ] Tests: archive/restore round trip over HTTP; archived gallery's client link, media and favorites are refused and work again after restore; delete returns 204, client link then 404, one cleanup job queued, files untouched until the worker runs; delete requires authentication and ownership; photographer can still upload to an archived gallery; existing photo-delete tests assert a cleanup job instead of file removal.
- [ ] Implement routes and update `galleries.rs` module docs and `photos.rs` docs.
- [ ] Run `cargo test -p pp-api` — PASS. Commit.

### Task 7: Web (`web/src`)

**Interfaces:**
- Produces: `archiveGallery(id)`, `restoreGallery(id)` returning `Gallery`, `deleteGallery(id)` returning `void` in `lib/gallery/api.ts`; `GalleryActions.svelte` (props: `archived: boolean`, `onarchive`, `onrestore`, `ondelete`, `busy`) rendered at the end of the side nav of the gallery page.

- [ ] Tests: api functions hit the right method and path and surface errors; component shows Archive, or Restore when archived, and Delete; page updates the gallery states after archive/restore and navigates to `/dashboard` after delete; 401 goes to the login page.
- [ ] Implement. Delete has no confirmation yet.
- [ ] Run `pnpm --dir web verify` — PASS. Commit.

### Task 8: Verification and delivery

- [ ] `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `pnpm --dir web verify`.
- [ ] Fresh independent review; address findings; refactor pass; remove `docs/plans/`; push `HEAD:feat/73-gallery-lifecycle`; open PR.
