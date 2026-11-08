# Client downloads Implementation Plan

> **For agentic workers:** implement task by task in this session (repo default), one fresh reviewer at the end.

**Goal:** Clients download one final or a prebuilt ZIP of all finals; the photographer sees downloads live.

**Architecture:** `exports` gets a `kind` (selection | delivery) and `downloaded_at`; `photos` gets `downloaded_at`. A delivery export is built by the existing worker job, freshness is the #220 hash compared on read, and the maintenance sweep removes delivery ZIPs of blocked galleries.

**Tech Stack:** Rust (actix, sqlx, worker queue), SvelteKit.

**Spec:** `docs/designs/2026-11-08-client-downloads-design.md`

## Global Constraints

- Selection ZIPs: 15 min, photographer only, unchanged behaviour. No free functions in non-test Rust.
- A stale delivery ZIP is never served; bytes are the uploaded bytes; downloaded = requested.
- Clients have no event stream; they poll.

## Review Focus

- A selection request must not delete or block the delivery ZIP (kind-scoped rules).
- Final removed/added while a build runs: the build is discarded, never served.
- Restore or extended expiry after the sweep removed the ZIP: next click rebuilds.
- Repeated click/ZIP request: `downloaded_at` unchanged, one event.
- Empty delivered set: no ZIP, no download-all.

---

### Task 1: Schema and kind-scoped export rules

**Files:** Create `crates/database/migrations/0020_client_downloads.sql`; modify `crates/database/src/export.rs`.

**Produces:** `ExportKind { Selection, Delivery }`; `exports.kind`, `exports.downloaded_at`, `photos.downloaded_at`; "one preparing" index, replace-previous, `latest`, `ready`, `delete_exports_in` scoped by kind; `expire(retention)` only selects `kind = 'selection'`.

- [ ] Write failing tests in `export.rs`: a selection request leaves a delivery export intact (and the reverse); `expire` skips delivery exports.
- [ ] Run them (`cargo test -p pp-database export`), expect FAIL.
- [ ] Migration + repository changes; existing callers pass `ExportKind::Selection`.
- [ ] Tests pass; commit.

### Task 2: Delivery export request, freshness and completion

**Files:** modify `crates/database/src/export.rs`, `gallery.rs` (deliver), `photo.rs` (ready/removal hooks).

**Produces:** `ExportRepository::request_delivery(workspace_id, gallery_id) -> ExportRequest` (snapshot of ready finals, hash, replaces previous delivery export in one transaction, enqueues `zip_export`; empty set creates nothing); `ExportRepository::current_delivery(gallery_id) -> DeliveryZip { Ready(ExportId) | Building | Empty }` (hash compare on read; creates a build if none current). `deliver` and final ready/removal on a delivered gallery call `request_delivery`. `complete` recomputes the hash for delivery kind and discards the build when stale.

- [ ] Failing tests: deliver creates one delivery export; adding/removing a final replaces it; stale never `Ready`; a completion with a moved set is discarded; empty set → `Empty`.
- [ ] Implement; tests pass; commit.

### Task 3: Recording downloads and events

**Files:** modify `export.rs`, `photo.rs`, migration 0020 (triggers), the SSE event mapping in `crates/api` (existing `export`/`photo` events).

**Produces:** `PhotoRepository::mark_downloaded(gallery_id, ids) -> ()` and `ExportRepository::mark_downloaded(id)` (first request only; ZIP marks every included final in one transaction); triggers announce ids on the photographer stream only.

- [ ] Failing tests: second request changes nothing; ZIP marks all its finals; event fires once; cross-workspace ids untouched.
- [ ] Implement; pass; commit.

### Task 4: Worker build and sweep

**Files:** modify `crates/image-worker/src/handlers/export.rs`, `maintenance.rs`.

- [ ] Failing tests: delivery build yields the finals' bytes in a ZIP; a stale build removes its file; sweep removes delivery ZIPs (row + file) of expired/archived galleries only and keeps live ones; orphan files still swept.
- [ ] Implement (reuse `ArchiveBuilder`, kind-aware plan); pass; commit.
- Note: the image-worker test build is broken on main (`pp_core::notifications` in `maintenance.rs:159`); fix that import if it blocks these tests.

### Task 5: Client routes

**Files:** modify `crates/api/src/routes/galleries/shared.rs`.

**Produces:** `GET /shared/{token}/photos/{id}/download` (final, ready, delivered only; attachment, `no-store`, `nosniff`); `GET /shared/{token}/download` (current ZIP → 200 attachment, else 202 `{"status":"building"}`, empty → 404). Both record downloads.

- [ ] Failing tests: byte-identical single and ZIP; proofs, drafts, other galleries, undelivered, expired, archived → 404; building → 202 then 200; repeats record once.
- [ ] Implement; pass; commit.

### Task 6: Web (client and photographer)

**Files:** modify the client gallery page/components and `PhotosPanel`/Finals tile wiring (`downloaded` prop exists on `PhotoTile`), `web/src/lib/gallery/*`.

- [ ] Failing tests: Download all shows "Downloading…", polls, starts the download once, toast "Couldn’t prepare the download. Please try again." on failure; no control for an empty set; per-photo download control; photographer tile mark on `downloaded_at` and live update on the event.
- [ ] Implement; `pnpm --dir web verify` passes; commit.

### Task 7: Docs and checks

- [ ] README implementation status and API notes; `docs/releases/MVP.md` boxes only if fully satisfied.
- [ ] `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace`, `pnpm --dir web verify`.
- [ ] Fresh reviewer, address findings, refactor pass, remove this plan, open PR (assignee @alekece), do not merge.
