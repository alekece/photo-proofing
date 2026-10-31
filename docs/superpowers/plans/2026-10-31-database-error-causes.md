# Credential-safe database error causes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connection, migration and repository failures expose a sanitized cause (never the database URL or password) through a `pp-database` error type, and `sqlx` types stop being public.

**Architecture:** `pp-database` gets `DatabaseError`, the single place that classifies `sqlx::Error`. `Database` gains `connect_lazy`, `ensure_reachable` and `migrate`; repositories and `GalleryError::Database` use `DatabaseError`; `api` maps it to `unavailable` and `ApiError::{Connection, Migration}` carry it as `source()`.

**Tech Stack:** Rust 2024, `sqlx` 0.8.6, `thiserror`, `eyre` (already at `api` `main`).

**Spec:** `docs/superpowers/specs/2026-10-31-database-error-causes-design.md` (issue #106)

## Global Constraints

- `thiserror` for the new type; `Display` describes only its own layer; the cause is exposed through `source()` (AGENTS.md).
- `Configuration` errors carry no detail: a fixed message only.
- Server `detail` is included only under `cfg(debug_assertions)`.
- Keep today's pool size of 10 connections.
- No `mod.rs`; imports at the top of the module; import parent modules for functions.
- Do not edit files with `sed`; use the edit tool.
- Checks: `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny --workspace check`. The 16 `pp-api` tests and new `#[sqlx::test]` tests need `DATABASE_URL` and run only in CI; report that.

## Review Focus

- A configuration error that echoes a URL value (`unknown value "s3cretpw" for ssl_mode`) must not reach `Display` or `source()` of `DatabaseError` or `ApiError`.
- A refused connection must report as unreachable immediately, not as a 30-second pool timeout.
- Release builds must never include the server `detail`.
- `GalleryError` length and character violations must still map to their settings (unchanged `From` logic).
- `ErrorResponse` for any database failure stays `unavailable` and never leaks the cause to clients.

---

### Task 1: `DatabaseError`

**Files:**
- Create: `crates/database/src/error.rs`
- Modify: `crates/database/src/lib.rs` (add `pub mod error;`)

**Interfaces:**
- Produces: `pub enum DatabaseError` in `pp_database::error` with kinds `Unreachable(#[source] io::Error)`, `Authentication(String)`, `Tls(String)`, `Configuration`, `Timeout`, `Migration(String)`, `Server { code: Option<String>, message: String }` (plus `detail` under `cfg(debug_assertions)`), `Other(String)`; `impl From<sqlx::Error>` and `impl From<sqlx::migrate::MigrateError>`.

- [ ] **Step 1: Write the failing tests** in `error.rs` `mod tests`:
  - `configuration_errors_never_echo_the_url`: `DatabaseError::from(sqlx::Error::Configuration("unknown value \"s3cretpw\" for ssl_mode".into()))`; assert `to_string()` and the whole `source()` chain do not contain `s3cretpw`.
  - `io_errors_are_unreachable_with_their_cause`: `sqlx::Error::Io(io::Error::from(ErrorKind::ConnectionRefused))` maps to `Unreachable`, and `source()` is `Some`.
  - `pool_timeouts_map_to_timeout`: `sqlx::Error::PoolTimedOut` maps to `Timeout`.
  - `#[sqlx::test] server_errors_keep_code_and_message`: run `SELECT 1/0` on the test pool, convert the error, assert it is `Server` with code `22012`.
- [ ] **Step 2: Run** `cargo test -p pp-database --locked error::` — expect FAIL (type not defined).
- [ ] **Step 3: Implement** `DatabaseError` and the two `From` impls in `error.rs`. `From<sqlx::Error>` is the only classifier: `Io`→`Unreachable`, `Tls`→`Tls` (message), `Configuration`→`Configuration`, `PoolTimedOut`→`Timeout`, server errors with SQLSTATE class `28` → `Authentication` (message), other server errors → `Server` (the `detail` field only under `cfg(debug_assertions)`), everything else → `Other` (message). `MigrateError` → `Migration` (message). Doc comment states the invariant: nothing here can contain the database URL.
- [ ] **Step 4: Run** the same command — expect PASS (DB-backed test only in CI).
- [ ] **Step 5: Commit** `feat(database): add credential-safe DatabaseError (#106)`.

### Task 2: `Database` connection API and repositories

**Files:**
- Modify: `crates/database/src/lib.rs`, `crates/database/src/auth.rs` (return types at lines 69-189), `crates/database/src/gallery.rs`, `crates/database/src/gallery/share.rs`

**Interfaces:**
- Consumes: `DatabaseError` from Task 1.
- Produces: `Database::connect_lazy(url: &str, max_connections: u32) -> Result<Database, DatabaseError>` (parses options, builds the pool without I/O, keeps the options), `Database::ensure_reachable(&self) -> Result<(), DatabaseError>` (one standalone connection from the stored options, then closed), `Database::migrate(&self) -> Result<(), DatabaseError>` (runs `MIGRATOR`). Repository methods return `Result<_, DatabaseError>`. `GalleryError::Database(#[source] DatabaseError)`.

- [ ] **Step 1: Write the failing test** in `lib.rs` `mod tests`: `ensure_reachable_reports_a_refused_connection_immediately` — `Database::connect_lazy("postgres://user:s3cretpw@127.0.0.1:1/pp", 1)` then `ensure_reachable().await` returns `Err(DatabaseError::Unreachable(_))` and `to_string()` plus chain never contain `s3cretpw`. (No database needed.)
- [ ] **Step 2: Run** `cargo test -p pp-database --locked ensure_reachable` — expect FAIL.
- [ ] **Step 3: Implement** the three `Database` methods in `lib.rs`; change the 8 `auth.rs` signatures and the `gallery.rs`/`share.rs` repository signatures from `sqlx::Error` to `DatabaseError` (bodies keep using `?`). Update `From<sqlx::Error> for GalleryError`: it still inspects the database error for the length/character constraint violations, and falls back to `GalleryError::Database(DatabaseError::from(error))`. Remove the `SqlError` re-export and the `PgPoolOptions` re-export; keep `PgPool` and `MIGRATOR`'s use internal.
- [ ] **Step 4: Run** `cargo test -p pp-database --locked` and `cargo clippy -p pp-database --all-targets --locked -- -D warnings` — expect PASS.
- [ ] **Step 5: Commit** `refactor(database): return DatabaseError and add connect_lazy/ensure_reachable/migrate (#106)`.

### Task 3: `api` wiring and verification

**Files:**
- Modify: `crates/api/src/api.rs` (`ApiError`, `Api::init`, the two tests), `crates/api/src/error.rs` (`From<SqlError>` → `From<DatabaseError>`)

**Interfaces:**
- Consumes: Task 2's `Database` methods and `DatabaseError`.
- Produces: `ApiError::Connection(#[source] DatabaseError)`, `ApiError::Migration(#[source] DatabaseError)`, still with no `From` impl (`#[from(skip)]` kept).

- [ ] **Step 1: Replace the test** `connection_and_migration_errors_expose_no_source` with `connection_and_migration_errors_expose_a_sanitized_cause`: build each variant from a `DatabaseError` made from a configuration error echoing `s3cretpw`; assert `source()` is `Some` and neither `to_string()` nor the source chain contains the password.
- [ ] **Step 2: Run** `cargo test -p pp-api --locked api::tests` — expect FAIL (variants carry no data).
- [ ] **Step 3: Implement** the variant changes; rewrite the `ApiError` doc comment (the cause is carried because `DatabaseError` is sanitized); `Api::init` becomes `Database::connect_lazy(config.database.url.as_str(), 10).map_err(ApiError::Connection)?`, then `ensure_reachable().map_err(ApiError::Connection)?`, then `migrate().map_err(ApiError::Migration)?`; change `error.rs` to `From<DatabaseError>`.
- [ ] **Step 4: Run** `cargo test -p pp-api --locked api::tests`, the fmt command, workspace clippy, and `cargo deny --workspace check` — expect PASS. Then run the binary with `PP_DATABASE_URL` set to an unreachable port, an unknown host, and `?sslmode=s3cretpw` (all with password `s3cretpw`): each prints a useful cause once and never the password.
- [ ] **Step 5: Commit** `feat(api): report a sanitized database cause at startup (#106)`.

### Task 4: Review and PR

- [ ] **Step 1:** Dispatch a fresh Opus code-review subagent against the design and the Review Focus; fix material findings and re-run the checks.
- [ ] **Step 2:** Remove this plan file from the branch (`git rm`) and commit.
- [ ] **Step 3:** Push and open the PR with `GH_TOKEN=$(~/.agents/bin/gh-app-token) gh pr create --assignee alekece`, body containing `Closes #106`, a summary, the verification evidence, and the note that DB-backed tests ran only in CI. Confirm CI is green, then ask for merge approval; squash-merge only after it is given and clean up the worktree.
