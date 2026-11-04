# Photos tab upload queue and single delete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Committed as `docs/plans/2026-11-04-photo-upload-ui.md` in Task 0. Issue #102, layer 2 of the #80 → #102 → #146 stack.
Local branch `alekece-feat/102-photo-upload-ui` (app prefix), pushed as `feat/102-photo-upload-ui`; PR base `feat/80-live-photo-events`.

**Goal:** The photographer drops JPEGs on the Photos tab, sees one tile per file moving through upload, processing and ready live, can retry recoverable failures and delete any photo at any moment.

**Architecture:** API gains a limits route, distinct 429/507 upload answers and a status-agnostic delete route. The web app gets an app-wide `UploadQueue` (runes class) owned by the dashboard layout, a `PhotosPanel` that merges the server list, queue items and #80 live events, and a `PhotoTile`.

**Tech Stack:** Rust (actix-web, sqlx/Postgres, pp-storage), SvelteKit 5 runes, vitest + testing-library + vitest-axe.

**Spec:** `docs/designs/2026-11-04-photo-upload-ui-design.md` (copied from the approved session design).

## Global Constraints

- Concurrency comes only from `GET /api/v1/photo-upload/limits` (`photo_upload.concurrency`), never hard-coded.
- No slot free → **429** + `Retry-After`; storage full → **507**; other 503s unchanged.
- Delete accepted whatever the status; 204 on success, 404 outside workspace/gallery.
- Files accepted client-side if `type === 'image/jpeg'` or name ends `.jpg`/`.jpeg` (case-insensitive); server authoritative.
- Confirmation only for photos already on the server; queued/uploading files go at once.
- Live updates via `getLiveEvents()` from `#lib/events.js` (`onPhoto`, `onResync`).
- No percentages; one looping bar for Uploading/Processing, still bar for waiting states.
- User code style: blank line before control flow, after `let` groups, before returns (unless alone in scope).

## Review Focus

1. ✕ on an uploading tile whose 201 arrives anyway → the queue deletes the new photo (queue test `cancel_after_created_deletes_photo`).
2. Two galleries with uploads at once → each panel only shows its own files; 404 drops only that gallery's files (queue test `gone_drops_only_that_gallery`).
3. A `deleted` event for a photo already removed optimistically, and a `photo` event for an unknown id while uploads are in flight → no flicker, no extra refetch (panel tests).
4. Drop containing non-files or folders/other types → only valid JPEGs upload; others become "Not a JPEG" tiles (queue test `rejects_non_jpeg_and_too_large_without_sending`).
5. Delete during processing → worker leaves no derivative (worker test `deleting_a_photo_mid_processing_leaves_nothing`).

---

### Task 0: Docs setup

- [ ] Copy the design to `docs/designs/2026-11-04-photo-upload-ui-design.md`, this plan to `docs/plans/2026-11-04-photo-upload-ui.md`; commit `docs: photo upload UI design and plan (#102)`.

### Task 1: Busy (429) and storage full (507) upload answers

**Files:** `crates/api/src/error.rs`, `crates/api/src/upload/limiter.rs`, tests in both and `routes/galleries/photos.rs`.

**Interfaces — Produces:** `ErrorResponse::busy(retry_after: Duration)` (429, message `"upload capacity busy"`, sets `Retry-After` in whole seconds, min 1); `ErrorResponse::storage_full()` now 507 `INSUFFICIENT_STORAGE`. Field `retry_after: Option<Duration>` `#[serde(skip)]`, header written in `error_response`. Limiter constant `BUSY_RETRY_AFTER = Duration::from_secs(1)`.

- [ ] Tests (fail first): limiter test asserts `StatusCode::TOO_MANY_REQUESTS` and `Retry-After: 1` on the error response; error test asserts DiskFull → 507 `"photo storage is full"`, other storage errors stay 503; route test `upload_without_free_slot_answers_429_with_retry_after` (concurrency 1, hold a streaming upload, second upload → 429 + header). Missing limiter stays 503.
- [ ] Implement, run `cargo test -p pp-api upload error`, commit `feat(api): answer busy uploads with 429 and full storage with 507 (#102)`.

### Task 2: Upload limits route

**Files:** Create `crates/api/src/routes/photo_upload.rs`; modify `routes.rs`, `api.rs::configure_routes`.

**Interfaces — Produces:** `GET /api/v1/photo-upload/limits` (`Authenticated`), `no_store` JSON `{"concurrency": n, "max_size": n, "max_pixels": n}` from `state.upload_config()`.

- [ ] Tests: `limits_require_a_session` (401), `limits_return_the_upload_config` (with `upload_config(1024, 2048)` → `{concurrency:4,max_size:1024,max_pixels:2048}`).
- [ ] Implement, test, commit `feat(api): expose photo upload limits (#102)`.

### Task 3: Delete a photo whatever its status

**Files:** `crates/database/src/photo.rs` (repository + tests), `crates/api/src/routes/galleries/photos.rs` (route + tests), `crates/image-worker/src/handlers/process_photo.rs` (test only).

**Interfaces — Produces:** `PhotoRepository::delete(&self, workspace_id, gallery_id, photo_id) -> Result<bool, DatabaseError>`: one transaction, `DELETE FROM photos WHERE id=$3 AND gallery_id=$2 AND workspace_id=$1`; 0 rows → rollback, `false`; else `DELETE FROM jobs WHERE payload->'payload'->>'photo_id' = $id` (any state), commit, `true`. Route `#[delete("/{id}/photos/{photo_id}")]` → 204 `no_store`, 404 when `false`; after commit removes `Original`, `Thumbnail`, `Preview` keys, `StorageError::NotFound` ignored, others `tracing::error!` and still 204.

- [ ] DB tests: `delete_removes_photo_and_all_its_jobs` for each of waiting/processing/ready/failed (queued, running and failed job rows gone); `delete_is_scoped_to_workspace_and_gallery` (false, row kept); `delete_announces_deleted` (listener gets `PhotoChange::Deleted`).
- [ ] API tests: `delete_removes_photo_and_its_files` (ready photo with stored derivatives → 204, files gone, list empty, thumbnail 404); `delete_requires_authentication_and_ownership` (401, foreign workspace 404, other gallery 404, unknown id 404); `delete_with_missing_files_still_succeeds`.
- [ ] Worker test: `deleting_a_photo_mid_processing_leaves_nothing` — claim, `PhotoRepository::delete`, then `publish` → completion not current, no thumbnail/preview files.
- [ ] Implement, test with Postgres, commit `feat(api): delete a photo whatever its status (#102)`.

### Task 4: Web photo API client

**Files:** Create `web/src/lib/gallery/photos.ts`, `photos.test.ts`.

**Interfaces — Produces:**
- `type PhotoStatus = 'waiting' | 'processing' | 'ready' | { failed: string }`; `interface Photo { id; original_name; size; width; height; status: PhotoStatus; created_at }`; `interface UploadLimits { concurrency; max_size; max_pixels }`.
- `listPhotos(galleryId, fetcher?) : Promise<Photo[]>`, `deletePhoto(galleryId, photoId, fetcher?) : Promise<void>`, `retryPhoto(galleryId, photoId, fetcher?) : Promise<void>`, `getUploadLimits(fetcher?) : Promise<UploadLimits>` — throw `GalleryApiError` (reused from `api.ts`) on non-OK.
- `uploadPhoto(galleryId, file, signal, fetcher?) : Promise<UploadOutcome>`; `UploadOutcome = {kind:'created', photo} | {kind:'busy', retryAfterMs} | {kind:'storage-full'} | {kind:'rejected', reason} | {kind:'failed'} | {kind:'gone'} | {kind:'signed-out'}`. Sends body `file`, headers `Content-Type: image/jpeg`, `X-Filename: encodeURIComponent(file.name)`. Mapping: 201 created; 429 busy (`Retry-After` seconds ×1000, default 1000); 507 storage-full; 413 "Too large"; 415 "Not a JPEG"; 422 "Unreadable image"; 404 gone; 401 signed-out; other status or network error failed; abort rethrows `AbortError`.
- `thumbnailUrl(galleryId, photoId)`, `previewUrl(galleryId, photoId)`.
- [ ] Tests: one table test per status mapping, headers/encoding, abort propagates. Implement, `pnpm --dir web test photos`, commit `feat(web): photo API client (#102)`.

### Task 5: App-wide upload queue

**Files:** Create `web/src/lib/gallery/uploads.svelte.ts`, `uploads.test.ts`; modify `web/src/routes/dashboard/+layout.svelte`, `layout.test.ts`.

**Interfaces — Produces:**
- `type QueueState = 'waiting' | 'uploading' | 'failed' | 'rejected'`; `interface UploadItem { key: string; galleryId: string; file: File; state: QueueState; error?: string }` (`error`: "Upload failed" | "Storage full" | rejection reason).
- `class UploadQueue` — constructor deps `{ upload?, limits?, remove?, onSignedOut?, now?, schedule? }` (defaults: `uploadPhoto`, `getUploadLimits`, `deletePhoto`, goto `/login?expired=1`). Members: `items` ($state), `add(galleryId, files: Iterable<File>)`, `retry(key)`, `cancel(key)`, `forGallery(galleryId): UploadItem[]`, `inFlight(galleryId): number` (waiting+uploading), `hasPending: boolean`, `onUploaded(handler: (galleryId, photo) => void): () => void`, `guard(event: BeforeUnloadEvent)` (preventDefault only while `hasPending`), `clear()`.
- `setUploadQueue(queue)`, `getUploadQueue()` (context).
- Behaviour: limits fetched once on first send (failure → 1 slot, refetch next send); client rejection before sending (type/ext → "Not a JPEG", size > `max_size` → "Too large"); at most `concurrency` uploads; created → item removed and `onUploaded` handlers called; busy → back to waiting with `notBefore`, after `MAX_BUSY_RETRIES = 5` consecutive → failed "Upload failed"; storage-full → that item and every waiting item → failed "Storage full"; gone → drop that gallery's items; signed-out → `clear()` + `onSignedOut`; cancel → abort + remove, and if the request still resolves `created`, call `remove(galleryId, photo.id)`.
- Layout: `const uploads = new UploadQueue(); setUploadQueue(uploads)`, `<svelte:window onbeforeunload={(e) => uploads.guard(e)} />`, `onDestroy(() => uploads.clear())`.
- [ ] Tests (fake `upload` with deferred promises, fake timers): `adds_one_item_per_file_in_drop_order`; `never_exceeds_server_concurrency`; `rejects_non_jpeg_and_too_large_without_sending`; `busy_requeues_after_retry_after_then_fails_after_five`; `storage_full_fails_the_rest`; `gone_drops_only_that_gallery`; `signed_out_clears_and_redirects`; `cancel_waiting_and_uploading`; `cancel_after_created_deletes_photo`; `retry_requeues_failed`; `guard_only_while_pending`; layout: `queue_survives_page_changes_and_clears_on_destroy`.
- [ ] Implement, test, commit `feat(web): app-wide photo upload queue (#102)`.

### Task 6: Photos panel and tiles

**Files:** Create `web/src/lib/components/PhotoTile.svelte`, `PhotosPanel.svelte`, `PhotosPanel.test.ts`; modify `GalleryTabs.svelte` (new prop `galleryId`, Photos tab renders `PhotosPanel`; Favorites/Finals unchanged), `routes/dashboard/[id]/+page.svelte` (pass `galleryId`), affected page tests.

**Interfaces:**
- `PhotoTile` props `{ name: string; state: TileState; error?: string; thumbnailUrl?: string; onremove: () => void; onretry?: () => void; onopen?: () => void }`, `TileState = 'waiting-upload' | 'uploading' | 'upload-failed' | 'rejected' | 'waiting-process' | 'processing' | 'process-failed' | 'ready'`. Labels per design table; ✕ button `aria-label="Remove {name}"`, top right, visible on hover/focus-within and on `(hover: none)`; Retry round button bottom right `aria-label="Retry {name}"`; looping bar via CSS animation, honoring `prefers-reduced-motion`; ready shows lazy `<img loading="lazy">` button opening the viewer.
- `PhotosPanel` props `{ galleryId: string }`; consumes `getUploadQueue()`, `getLiveEvents()`, Task 4 client.
  - Loads `listPhotos`; tiles = server photos (server order) then `queue.forGallery` (drop order), keyed by photo id / queue key.
  - `onUploaded` for this gallery → append photo if unknown. `onPhoto` for this gallery: `deleted` → drop; known id → update status (`failed` → `{failed: 'Processing failed'}`); unknown id → ignore while `queue.inFlight(galleryId) > 0`, else refetch. `onResync` → refetch. Refetch throttled to once per 1000 ms; refetch once when `inFlight` drops to 0.
  - Delete server photo: `Dialog` confirm → optimistic removal → `deletePhoto`; error other than 404 → restore + `role="alert"` "Couldn’t delete {name}."; 401 → login. Queue tiles → `queue.cancel` without dialog. Process retry → `retryPhoto`, status waiting optimistically. Upload retry → `queue.retry`.
  - Whole panel is a drop target (`dragover` preventDefault, `drop` → `queue.add`); empty state is the mockup drop zone "No photos yet. Drop JPEGs here to start."; `+ Upload` button opens hidden `<input type="file" multiple accept="image/jpeg,.jpg,.jpeg">`.
  - Ready tiles open `ProofViewer` with `Proof`s built from ready photos (`thumbnailUrl`/`previewUrl` helpers).
  - Polite live region with counts, e.g. "2 uploading, 3 processing, 1 failed".
- [ ] Tests: `drop_creates_tiles`; `each_state_shows_its_label_and_actions` (table); `events_move_tiles_to_ready_and_remove_deleted`; `unknown_event_during_uploads_does_not_refetch`; `resync_refetches_at_most_once_a_second`; `remove_queued_without_confirmation`; `remove_server_photo_asks_then_deletes_and_restores_on_failure`; `retry_processing_failed_calls_api`; `ready_tile_opens_viewer`; `axe_passes`.
- [ ] Implement, `pnpm --dir web verify`, commit `feat(web): photos tab with upload tiles and delete (#102)`.

### Task 7: Docs, checks, review, PR

- [ ] README: status line (upload UI now exists for the Photos tab; drop "upload UI … not implemented" and "upload photos through the API until…"), admission paragraph 503 → 429 + `Retry-After`, mention 507 for full storage, the limits and delete routes. Update `docs/designs/2026-10-31-photo-upload-design.md` status table rows in place (admission 429, storage full 507).
- [ ] Postgres: `COMPOSE_PROJECT_NAME=pp102 PP_POSTGRES_HOST_PORT=<free port>`; `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `pnpm --dir web verify`.
- [ ] Fresh independent reviewer (correctness + design compliance) → fix material findings → refactor pass → rerun checks.
- [ ] Push `git -c credential.helper= -c credential.helper='!gh auth git-credential' push -u origin HEAD:feat/102-photo-upload-ui`; `gh pr create --base feat/80-live-photo-events --head feat/102-photo-upload-ui --assignee alekece`, concise human description, "Closes #102", "Stacks on #147". Report PR number to the coordinator session. Don't merge.
