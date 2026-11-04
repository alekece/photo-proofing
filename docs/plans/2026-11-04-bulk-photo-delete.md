# Select mode and bulk photo delete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

Committed as `docs/plans/2026-11-04-bulk-photo-delete.md` in Task 0. Issue #146, layer 3 of the #80 → #102 → #146 stack.
Local branch `alekece-feat/146-bulk-photo-delete` (app prefix), pushed as `feat/146-bulk-photo-delete`; PR base `feat/102-photo-upload-ui` (#148).

**Goal:** On the Photos tab the photographer enters select mode, picks photos (or all), confirms once, and one request per 1000 photos deletes them with the guarantees of the single delete.

**Architecture:** `PhotoRepository::delete` takes a slice of ids and returns the ids it removed; the single and bulk routes share it and a file clean-up helper. A new `POST /api/v1/galleries/{id}/photos/delete` resource carries its own JSON limit. The web gets `deletePhotos()`, a selectable `PhotoTile`, and select mode in `PhotosPanel`.

**Tech Stack:** Rust (actix-web, sqlx/Postgres, pp-storage), SvelteKit 5 runes, vitest + testing-library + vitest-axe.

**Spec:** `docs/designs/2026-11-04-bulk-photo-delete-design.md`.

## Global Constraints

- Route `POST /api/v1/galleries/{id}/photos/delete`, body `{"photo_ids": [...]}`, answer `200 {"deleted": n}` with `Cache-Control: no-store`; session and gallery ownership required.
- Ids outside this workspace/gallery or already gone are ignored and not counted; a repeated id counts once.
- Empty list or more than **1000** ids → 400; unknown/foreign gallery → 404; no session → 401.
- JSON limit for this route sized for 1000 ids: **64 KiB** (shared limit stays 4096).
- Same transaction as single delete: jobs locked/deleted before photos (worker order); files removed after commit, failures only logged.
- `/photos/delete` registered before the `{photo_id}` routes.
- Web: queued browser files are never selectable; select all takes the server photos shown at that moment; Esc leaves select mode and clears selection; Delete disabled at 0; dialog copy "Delete n photos?" / "This can't be undone."; chunks of 1000 sent sequentially.
- User code style: blank line before control flow, after `let` groups, before returns (unless alone in scope).

## Decisions taken while planning (flag in PR)

- **Body type errors keep the shared JSON mapping** (malformed JSON 400, a non-UUID or wrong-typed value 422, oversize 413), as every other JSON route does. The design text said 400 for "not a list of UUIDs"; the copied design is adjusted to say this. Count rules (empty, >1000) are 400 as designed.
- **"Toast"** is the panel's existing `role="alert"` line; there is no toast component.
- **Lock order with several photos:** jobs locked `ORDER BY id FOR UPDATE` then deleted, then photos locked `ORDER BY id FOR UPDATE` then deleted. Keeps the worker's jobs-then-photo order and makes two overlapping bulk deletes lock in the same order.
- **Ctrl/⌘+A only works inside select mode** (outside it, the browser's native select-all stays).
- Shared delete keeps the name `PhotoRepository::delete`, now `(&self, WorkspaceId, GalleryId, &[PhotoId]) -> Result<Vec<PhotoId>, DatabaseError>`.

## Review Focus

1. Exactly 1000 ids, pretty-printed JSON → accepted (limit sizing); 1001 → 400. API test `bulk_delete_accepts_1000_ids_and_rejects_more`.
2. A `deleted` live event for a selected photo (other tab) → it leaves the selection and the count; the confirmed count matches what is sent. Panel test `drops_a_photo_deleted_elsewhere_from_the_selection`.
3. A chunk whose answer is lost (fetch rejects) → no further chunks, list refetched, survivors stay selected, alert with deleted count. Panel test `stops_and_refetches_when_a_chunk_fails`.
4. Esc while the confirmation dialog is open → closes only the dialog; select mode and selection stay. Panel test `esc_in_the_dialog_keeps_select_mode`.
5. A photo uploaded after Select all → not selected. Panel test `select_all_takes_only_photos_shown_now`.

---

### Task 0: Docs setup

- [ ] Copy the approved design to `docs/designs/2026-11-04-bulk-photo-delete-design.md`; change the route bullet's validation sentence to: "An empty list or more than 1000 ids is 400 and a body that is not a list of UUIDs is rejected like any JSON body (400 malformed, 422 wrong type), with a JSON limit sized for 1000; an unknown gallery is 404." and "a toast says" → "the panel's alert says".
- [ ] Copy this plan to `docs/plans/2026-11-04-bulk-photo-delete.md`.
- [ ] Commit `docs: bulk photo delete design and plan (#146)`.

### Task 1: Shared multi-photo delete in the database

**Files:**
- Modify: `crates/database/src/photo.rs` (`delete` at ~270, tests at ~925–1060)

**Interfaces:**
- Produces: `PhotoRepository::delete(&self, workspace_id: WorkspaceId, gallery_id: GalleryId, ids: &[PhotoId]) -> Result<Vec<PhotoId>, DatabaseError>` — removed ids, each once; empty `ids` returns empty without a query.

- [ ] **Step 1: Adapt existing tests to the slice API and add**
  - `delete_removes_several_photos_and_ignores_others`: three photos in `location`, one in `other`; call with `[a, b, foreign, unknown, a]` → returned set `{a, b}`; `c` and foreign remain with their 1 job; `a`,`b` have no jobs.
  - existing three tests: `delete(.., &[id])` asserts `== vec![id]` / `is_empty()`.
- [ ] **Step 2:** `cargo test -p pp-database delete` → compile failure.
- [ ] **Step 3:** Implement: one transaction; `DELETE FROM jobs WHERE id IN (SELECT id FROM jobs WHERE payload->'payload'->>'photo_id' IN (SELECT id::text FROM photos WHERE workspace_id = $1 AND gallery_id = $2 AND id = ANY($3)) ORDER BY id FOR UPDATE)`, then `DELETE FROM photos WHERE id IN (SELECT id FROM photos WHERE workspace_id = $1 AND gallery_id = $2 AND id = ANY($3) ORDER BY id FOR UPDATE) RETURNING id`; commit only when something was deleted (as today). Update the doc comment (lock order, ignored ids).
- [ ] **Step 4:** Tests pass, including `delete_waits_for_a_worker_finishing_the_photo`.
- [ ] **Step 5:** Commit `refactor(database): delete several photos in one transaction (#146)`.

### Task 2: Bulk delete route

**Files:**
- Modify: `crates/api/src/routes/galleries/photos.rs`, `crates/api/src/api.rs` (JsonConfig at ~300), `README.md:222`

**Interfaces:**
- Consumes: Task 1 `delete`.
- Produces: `pub(crate) fn json_config(limit: usize) -> JsonConfig` in `api.rs` (shared error handler), used by `configure_routes` and the bulk resource; `async fn remove_files(storage, location: PhotoLocation, ids: &[PhotoId])` in `photos.rs` used by both delete routes; `const MAX_BULK_DELETE: usize = 1000`, `const BULK_DELETE_JSON_LIMIT: usize = 64 * 1024`.

- [ ] **Step 1: Failing tests** (helper `bulk_delete_request(cookie, gallery, body: serde_json::Value)` posting JSON with `Origin`):
  - `bulk_delete_removes_photos_jobs_and_files`: four photos (processing, ready, failed, waiting) with derivatives; delete three → `200 {"deleted":3}`, `no-store`, their files and jobs gone, fourth intact.
  - `bulk_delete_ignores_foreign_unknown_and_repeated_ids`: ids = [own, own, other gallery's photo, unknown] → `{"deleted":1}`; other gallery's photo keeps status and files.
  - `bulk_delete_accepts_1000_ids_and_rejects_more`: 1000 random UUIDs via `serde_json::to_string_pretty` → 200 `{"deleted":0}`; 1001 → 400; `[]` → 400; `{"photo_ids":"x"}` and `["not-a-uuid"]` → 422; non-JSON body → 400.
  - `bulk_delete_requires_authentication_and_ownership`: no cookie → 401; foreign-workspace gallery and unknown gallery → 404.
- [ ] **Step 2:** `cargo test -p pp-api bulk_delete` → fails (404 route).
- [ ] **Step 3:** Implement `delete_photos` handler (`Json<BulkDelete { photo_ids: Vec<PhotoId> }>` with `deny_unknown_fields`), gallery existence check via `GalleryRepository::get` (as `list_photos`), count check → `ErrorResponse::bad_request()`, `delete`, `remove_files`, `Deleted { deleted: u64 }`. Register in `configure` first as `web::resource("/{id}/photos/delete").app_data(json_config(BULK_DELETE_JSON_LIMIT)).route(web::post().to(delete_photos))`. Single `delete_photo` uses `delete(.., &[photo_id])` and `remove_files`. Update module doc and README line 222 to mention the bulk route.
- [ ] **Step 4:** `cargo test -p pp-api` passes.
- [ ] **Step 5:** Commit `feat(api): delete several photos in one request (#146)`.

### Task 3: Web client for bulk delete

**Files:**
- Modify: `web/src/lib/gallery/photos.ts`, `web/src/lib/gallery/photos.test.ts`

**Interfaces:**
- Produces: `export const BULK_DELETE_LIMIT = 1000;` and `export async function deletePhotos(galleryId: string, photoIds: string[], fetcher = fetch): Promise<number>` — POSTs `{"photo_ids": photoIds}` with `Content-Type: application/json` to `/api/v1/galleries/{id}/photos/delete`, returns `deleted`, throws `GalleryApiError` on non-2xx.

- [ ] Test `deletes several photos in one request` (URL, method, body, header, returned count) in `photo requests`; run `pnpm --dir web test photos` fail → implement → pass; commit with Task 4.

### Task 4: Select mode in the Photos tab

**Files:**
- Modify: `web/src/lib/components/PhotoTile.svelte`, `web/src/lib/components/PhotosPanel.svelte`, `web/src/lib/components/PhotosPanel.test.ts`

**Interfaces:**
- Consumes: Task 3 `deletePhotos`, `BULK_DELETE_LIMIT`.
- Produces: `PhotoTile` props `selected?: boolean` and `onselect?: () => void`. When `onselect` is set the tile is in select mode: a `<label>` covers it with a visible `<input type="checkbox" aria-label="Select {name}">`, the thumbnail is a plain `<img>` (no viewer button), ✕ and Retry are not rendered.

- [ ] **Step 1: Failing panel tests** (extend `stubApi` so `POST …/photos/delete` answers `{deleted}` from the body length or a given status/rejection):
  - `enters and leaves select mode`: `Select` button → checkboxes on server tiles only (a queued file has none), no ✕/Retry, clicking a tile toggles it and does not open the viewer; bar reads "1 selected"; Cancel and Esc both leave and clear.
  - `select_all_takes_only_photos_shown_now`: Select all (button and Ctrl+A, also ⌘/Meta+A) selects all shown; an upload answered afterwards is not selected; count unchanged.
  - `confirms once and deletes the selection`: Delete disabled at 0; dialog "Delete 2 photos?" + "This can't be undone."; confirm → tiles gone at once, one POST with both ids, select mode left.
  - `sends more than 1000 photos in chunks, one after another` (1001 photos → two POSTs of 1000 and 1, second only after first resolves).
  - `stops_and_refetches_when_a_chunk_fails`: 1001 selected, first chunk 500 → no second POST, list refetched, select mode open, refetched survivors still selected, alert "Deleted 0 of 1001 photos. Please try again." (copy: `Deleted ${done} of ${total} photos. Please try again.`).
  - `drops_a_photo_deleted_elsewhere_from_the_selection`: select 2, emit `deleted` for one → "1 selected", dialog says "Delete 1 photo?".
  - `esc_in_the_dialog_keeps_select_mode`.
  - 401 on a chunk → `goto('/login?expired=1', …)`.
  - axe test also runs in select mode with the bar shown.
- [ ] **Step 2:** `pnpm --dir web test PhotosPanel` fails.
- [ ] **Step 3:** Implement in `PhotosPanel`: `selecting` state, `selected = new SvelteSet<string>()`, `selection = $derived(photos.filter(p => selected.has(p.id)))` (so live deletions drop out), bar "n selected · Select all · Delete · Cancel" replacing the Upload row while selecting (Select button next to Upload, disabled with no server photos), `<svelte:window onkeydown>` handling Esc / Ctrl|Meta+A only when `selecting` and no dialog open. `removeSelection()`: snapshot ids, add to `deleting`, drop from `photos`, send `chunks of BULK_DELETE_LIMIT` sequentially; each success marks ids `recentChanges` deleted and removes them from `selected`; on failure (non-401) stop, clear `deleting` for unconfirmed ids, `await load()`, set alert; on full success leave select mode. Singular/plural copy for 1 photo.
- [ ] **Step 4:** `pnpm --dir web verify` passes.
- [ ] **Step 5:** Commit `feat(web): select mode and bulk photo delete (#146)`.

### Task 5: Verify, review, refactor, deliver

- [ ] Postgres on a free port with own project: `COMPOSE_PROJECT_NAME=pp146 PP_POSTGRES_HOST_PORT=<free, not 55432> docker compose up -d postgres`; `DATABASE_URL=… cargo test --workspace`; `cargo +nightly fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `pnpm --dir web verify`.
- [ ] Fresh independent reviewer (code-review agent) against design + plan; fix material findings; refactor pass; rerun checks; record result in PR.
- [ ] Push `git -c credential.helper= -c credential.helper='!gh auth git-credential' push -u origin HEAD:feat/146-bulk-photo-delete`; `gh pr create --base feat/102-photo-upload-ui --head feat/146-bulk-photo-delete --assignee alekece` — closes #146, stacks on #148, concise human description incl. the decisions above.
- [ ] Send PR number to the coordinator session. Plan removal from `docs/plans/` happens after review, before merge approval.
