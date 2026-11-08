# Short-lived selection ZIPs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Selection ZIPs expire 15 minutes after they are ready, and a request for an unchanged selection reuses the live ZIP.

**Architecture:** `ExportRepository::request_selected` compares the requested photo ids with the newest unexpired ready export's snapshot and returns it instead of creating one. Defaults for retention and sweep interval change; no schema change.

**Tech Stack:** Rust (sqlx, actix-web), `#[sqlx::test]` database tests.

**Spec:** `docs/designs/2026-11-08-short-lived-selection-zips-design.md` (issue #220).

## Global Constraints

- `export-retention` default `15m` (`PP_EXPORT_RETENTION`); `cleanup-sweep-interval` default `5m`.
- Reuse never extends the lifetime and needs no new column or migration.
- Follow `crates/AGENTS.md` (no non-test free functions; blank-line readability rules).

## Review Focus

- A ready export whose selection changed (favorite added or removed) is not reused.
- A failed, preparing or expired export is never reused.
- A subset request equal to the live export's photos is reused; a different subset replaces it.

---

### Task 1: Reuse an unchanged live ZIP

**Files:**
- Modify: `crates/database/src/export.rs` (`ExportRequest`, `request`, `request_selected`, tests)
- Modify: `crates/api/src/routes/galleries/export.rs` (`request_export`, tests)

**Interfaces:**
- Produces: `ExportRequest::Reused(Export)`; `request(&self, workspace_id, gallery_id, max_size: NonZeroU64, retention: Duration)` and `request_selected(&self, workspace_id, gallery_id, max_size, retention: Duration, selected: Option<&[PhotoId]>)`.
- Consumes: `ApiState::export_retention() -> Duration`.

- [ ] **Step 1: Write failing database tests** in `export.rs` tests, using the `Fixture`: `an_unchanged_selection_reuses_the_ready_export` (create, mark ready via the existing completion helper, request again: `Reused(export)` with the same id and one job in total), `a_changed_selection_is_not_reused` (toggle a favorite, expect `Created`), `an_expired_failed_or_preparing_export_is_not_reused`, `an_identical_subset_is_reused_and_another_subset_replaces_it`.
- [ ] **Step 2: Run** `cargo test -p pp-database export` and expect the new tests to fail to compile or fail.
- [ ] **Step 3: Implement** the new `Reused` variant and `retention` parameter. After the favorites are read and size-checked, and before `delete_exports_in`, select the gallery's newest export that is ready and has `finished_at` within `retention`; compare its `export_photos` ordered by `position` with the favorite ids; on equality return `Reused`. Update `request` to pass `retention` through.
- [ ] **Step 4: Run** `cargo test -p pp-database export`; expect PASS.
- [ ] **Step 5: API.** In `request_export` pass `state.export_retention()` and answer `Reused` with `200` and `ExportResponse::from(export)` (no_store). Add API test `requesting_an_unchanged_selection_returns_the_ready_export` asserting 200, same id, and a different id after a favorite changes. Run `cargo test -p pp-api export`; expect PASS.
- [ ] **Step 6: Commit** `feat: reuse an unchanged selection ZIP (#220)`.

### Task 2: Short lifetime defaults and docs

**Files:**
- Modify: `crates/core/src/config.rs` (`DEFAULT_EXPORT_RETENTION`, `export-retention` default, any config test), `crates/image-worker/src/config.rs` (`cleanup-sweep-interval` default and its tests)
- Modify: `README.md` (ZIP availability sentence and `PP_EXPORT_RETENTION` default), `docs/designs/2026-11-07-selection-zip-design.md` (bounded-storage paragraph)

- [ ] **Step 1: Write failing tests** asserting the parsed defaults: export retention is 15 minutes and sweep interval is 5 minutes (extend the existing config tests in both crates).
- [ ] **Step 2: Run** `cargo test -p pp-core -p pp-image-worker config`; expect FAIL.
- [ ] **Step 3: Change** the constants and `default_value`s to `15m` and `5m`; update doc comments ("unavailable and deleted once older than this").
- [ ] **Step 4: Docs.** README: ZIPs remain available for 15 minutes; `PP_EXPORT_RETENTION` default `15m`; mention reuse of an unchanged selection. Selection ZIP design: replace "default 24 h" and the "no workspace-wide quota (followed up)" wording with a pointer to the new design.
- [ ] **Step 5: Run** the full checks from `crates/AGENTS.md` (format, clippy, tests); expect all green.
- [ ] **Step 6: Commit** `feat: delete selection ZIPs after 15 minutes (#220)`.
