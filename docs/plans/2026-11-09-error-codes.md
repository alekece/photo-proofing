# Translatable API Errors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every API error carries a stable `code` the web translates, and export failures are stored as codes.

**Architecture:** `ErrorCode` (API crate only) is chosen by each `ErrorResponse` conversion. The database stores a typed `ExportFailure`; the API maps it to an `ErrorCode`. The web maps `code → m.*` explicitly, falling back to the status sentence.

**Tech Stack:** Rust (actix-web, sqlx, serde), SvelteKit, Paraglide, valibot.

**Spec:** `docs/designs/2026-11-09-error-codes-design.md` (issue #285)

## Global Constraints

- Body is `{"code": "<snake_case>", "context": [..English..]}`; `error` is removed; `context` is omitted in release builds (`cfg!(debug_assertions)`); status and `Retry-After` unchanged.
- Code list is the design's table, frozen by a test asserting the serialized names.
- Every "missing or not visible" case stays `not_found`.
- Domain crates (database, worker, storage) never import `ErrorCode`.
- Logs keep the detailed English text.
- Non-test Rust code has no free functions; blank-line readability rules from `AGENTS.md` apply.
- Checks: `crates/AGENTS.md` fmt/lint/test commands; `pnpm --dir web verify`.

## Review Focus

- Unknown or missing `code` from the API → web shows the status sentence, never crashes (Task 3).
- Legacy export rows with unrecognized text → read as `attempts_exhausted` (Task 1).
- A failed export in the old stored shape must still deserialize during a rolling deploy (Task 1 migration is the only writer fix; tests cover rows).
- Release build body has no `context` and leaks no internal detail (Task 2).
- French has every new message (Task 3, existing key-parity test).

---

### Task 1: Typed export failure in the database and worker

**Files:**
- Modify: `crates/database/src/export.rs` (remove `EXPORT_FAILED_REASON`, `PHOTO_REMOVED_REASON`; `ExportStatus::Failed`; `fail`, `mark_failed_in`, `complete` call sites; tests at ~1159, 1224, 1258, 1276, 1314, 1567)
- Modify: `crates/database/src/jobs.rs` (~787 uses `EXPORT_FAILED_REASON`)
- Modify: `crates/image-worker/src/handlers/export.rs` (`ExportOutput::Failed`, `DISK_FULL_REASON`, tests ~337, 366)
- Create: `crates/database/migrations/0023_export_failure_codes.sql`

**Interfaces:**
- Produces: `pub enum ExportFailure { AttemptsExhausted, PhotoRemoved, DiskFull }` (`serde(rename_all = "snake_case")`, `Copy`); `ExportStatus::Failed(ExportFailure)`; `ExportRepository::fail(&self, job: &Job, id: ExportId, failure: ExportFailure)`.

- [ ] **Step 1: Failing migration test** in `export.rs` tests, in the style of `jobs.rs:578-624`: insert rows with status `{"kind":"failed","reason":"<legacy sentence>"}` for each of the three legacy sentences plus `"boom"`, apply `0023`, assert reasons become `photo_removed`, `export_failed`→`attempts_exhausted`, `disk_full`, and `attempts_exhausted`.
- [ ] **Step 2: Run it** (`cargo test -p pp-database export`) — expect FAIL.
- [ ] **Step 3: Write `0023_export_failure_codes.sql`** rewriting `reason` of failed rows: `"A selected photo was removed. Try again."`→`photo_removed`, `"The server is out of disk space."`→`disk_full`, anything else→`attempts_exhausted`. Check the exact legacy disk text in `handlers/export.rs` `DISK_FULL_REASON`.
- [ ] **Step 4: Add `ExportFailure`, change `ExportStatus::Failed` and `fail`/`mark_failed_in`**, update the worker (`ExportOutput::Failed { id, failure: ExportFailure }`, retry exhaustion in `jobs.rs` uses `AttemptsExhausted`), and update existing tests to the new type.
- [ ] **Step 5: Run** `cargo test -p pp-database -p pp-image-worker` — expect PASS.
- [ ] **Step 6: Commit** `feat(database): store export failures as codes (#285)`.

### Task 2: `ErrorCode` and the new response body in the API

**Files:**
- Modify: `crates/api/src/error.rs` (enum, `ErrorResponse`, all constructors and `From` impls, tests)
- Modify: `crates/api/src/routes/auth.rs` (`AuthError::error_response`, ~70-75)
- Modify: `crates/api/src/routes/galleries/export.rs` (export status shape ~43), `shared.rs` (~309, ~333), `galleries.rs` (~255)
- Modify: other API tests asserting `"error"` (grep `"error"` in `crates/api/src`)

**Interfaces:**
- Consumes: `ExportFailure` from Task 1.
- Produces: `pub(crate) enum ErrorCode` with the design's table (snake_case serde); `ErrorResponse { code, context, .. }` serialized as `{"code", "context"?}`; `ErrorCode::from(ExportFailure)`; `AuthError::code(&self) -> ErrorCode`; the API export status serializes failures as `{"kind":"failed","code":"<ErrorCode>"}`.

- [ ] **Step 1: Failing tests** in `error.rs`: `error_codes_are_frozen` asserts the full serialized list equals the design table verbatim; `body_has_code_and_no_error_key`; `gallery_setting_errors_use_per_field_codes` (title→`invalid_title`, etc.) and settings likewise; `export_failure_maps_to_codes`; a `#[cfg(not(debug_assertions))]` test that `context` is absent; unit test that `not_found` is the one code for the not-found constructor.
- [ ] **Step 2: Run** `cargo test -p pp-api error` — expect FAIL.
- [ ] **Step 3: Implement** `ErrorCode`; make each constructor choose a code (replace `conflict(&'static str)` with explicit constructors `gallery_delivered`, `gallery_not_shared`, `zip_not_ready`, `zip_in_progress`, `selection_empty`); keep the English text as `context[0]`, dropped via `cfg!(debug_assertions)`; remove `error`; per-field codes replace the field name in `context`. Add the `AuthError` mapping (`too_many_attempts` for `Limited`) and render its body through `ErrorResponse`. Add any code for a case the table misses and update the design table.
- [ ] **Step 4: Run** `cargo test -p pp-api` plus the `crates/AGENTS.md` fmt and clippy commands — expect PASS.
- [ ] **Step 5: Commit** `feat(api): return stable error codes (#285)`.

### Task 3: Web translation, docs and cleanup

**Files:**
- Modify: `web/src/lib/i18n/errors.ts` (+ `errors.test.ts`), `web/src/lib/settings/settings.ts` (~133), `web/src/lib/gallery/api.ts` (~36), `web/src/lib/gallery/{export,photos}.ts`, `web/src/lib/auth.ts` (pass the code), `web/src/lib/gallery/schemas.ts` (~152), `web/src/lib/components/FavoritesPanel.svelte` (~132)
- Modify: `web/messages/en.json`, `web/messages/fr.json`
- Modify: `README.md` (remove the #284 limitation), delete `docs/plans/2026-11-09-error-codes.md` before final merge request

**Interfaces:**
- Produces: `apiErrorMessage(status: number, code?: string): string` (explicit `code → m.*` table, then status table, then `m.api_error_generic`); `readErrorCode(data: unknown): string | undefined`.

- [ ] **Step 1: Failing tests** in `errors.test.ts`: known code beats status (`zip_not_ready` with 409), unknown code falls back to the status sentence, missing code and unknown status → generic, every table code has a message in both languages; a `schemas` test that `failed` accepts any string `code` and an unknown `code`.
- [ ] **Step 2: Run** `pnpm --dir web test errors` — expect FAIL.
- [ ] **Step 3: Implement**: table in `errors.ts` (field messages move under per-field codes), callers pass `code` read from the body instead of `context[0]`, schema `v.object({kind: 'failed', code: v.string()})`, FavoritesPanel shows `apiErrorMessage(409, code)`-style message for a failed export; add the new messages (English and French) for every code in the design table that lacks one.
- [ ] **Step 4: Run** `pnpm --dir web verify` — expect PASS.
- [ ] **Step 5: README**: remove the limitation sentence. Run the checks of Tasks 1-2 once more.
- [ ] **Step 6: Commit** `feat(web): translate API error codes (#285)`.
