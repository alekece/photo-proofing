# thiserror Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move all `derive_more` error types to `thiserror` with layer-only messages, and report startup errors with the full chain via `color-eyre`.

**Architecture:** Mechanical derive swap per crate, then `main` returns `eyre::Result<()>`.

**Tech Stack:** thiserror 2, eyre, color-eyre, cargo-deny.

**Spec:** `docs/superpowers/specs/2026-10-31-thiserror-migration-design.md` (issue #103)

## Global Constraints

- `ErrorResponse` and `AuthError` messages do not change; `eyre` only in `crates/api/src/main.rs`.
- `ApiError::Connection` and `ApiError::Migration` have no source.
- Keep existing `From` impls on exactly the same variants (`ApiError`: `SessionLifetime`, `Hashing`, `Serve`; `ParseBoundedDurationError`: both).
- `GalleryError::Database(sqlx::Error)` stays as is apart from the derive and message.

## Review Focus

- A `From` impl lost or gained by the derive swap (`#[from]` placement).
- A message that still embeds its cause next to a `source`.
- `ApiError::Connection`/`Migration` accidentally gaining a source (credential leak).
- `cargo deny` failing on the new dependencies' licenses.

---

### Task 1: core and database

**Files:** Modify `Cargo.toml` (workspace deps), `crates/core/Cargo.toml`, `crates/database/Cargo.toml`, `crates/core/src/{email,url,time}.rs`, `crates/core/src/types/token.rs`, `crates/database/src/gallery.rs`

- [ ] **Step 1:** Add `thiserror.workspace = true` to `core` and `database`.
- [ ] **Step 2:** In each file replace `Display, Error` derives with `thiserror::Error` and `#[display]` with `#[error]`. Embedded causes leave the message and the cause becomes `#[from]`/`#[source]`: `ParseBoundedDurationError` (`#[from]` on both variants, messages `invalid duration` and transparent `#[error(transparent)]` for `Range`), `GalleryError::Database` (`#[error("database error")]`, `#[source]`). `DurationRangeError` keeps its message.
- [ ] **Step 3:** Run `cargo test -p pp-core --locked`. Expected: pass. Run `cargo clippy -p pp-core -p pp-database --all-targets --locked -- -D warnings`. Expected: clean.
- [ ] **Step 4:** Commit `refactor: use thiserror in core and database (#103)`.

### Task 2: api errors and test

**Files:** Modify `crates/api/src/{api,config,credentials}.rs`; test in `crates/api/src/api.rs`

- [ ] **Step 1:** Write test `connection_and_migration_errors_expose_no_source` asserting `std::error::Error::source(&ApiError::Connection).is_none()` and same for `Migration`. Run it; Expected: PASS already (guards the invariant).
- [ ] **Step 2:** Swap derives in the three files. `ApiError`: `#[from]` on `SessionLifetime`, `Hashing`, `Serve` (messages without the embedded cause); `Connection`/`Migration` keep messages, no source, no `From`.
- [ ] **Step 3:** Run `cargo test -p pp-api --locked` (database tests need PostgreSQL; report if unavailable) and clippy. Expected: pass.
- [ ] **Step 4:** Commit `refactor: use thiserror in api (#103)`.

### Task 3: eyre at main, guidance, deny

**Files:** Modify `Cargo.toml`, `crates/api/Cargo.toml`, `crates/api/src/main.rs`, `AGENTS.md`, `deny.toml` (only if required)

- [ ] **Step 1:** Add `eyre` and `color-eyre` to `[workspace.dependencies]` and `crates/api`. `main` returns `eyre::Result<()>`, calls `color_eyre::install()?` first, and propagates `Api::init(..).await?.run().await?`; drop the `IoError` mapping and unused imports.
- [ ] **Step 2:** Run `cargo deny --workspace check`. Expected: all ok; if licenses fail, add only the required ones.
- [ ] **Step 3:** `AGENTS.md`: state `eyre` reports belong only at a binary's `main`, and remove "existing `derive_more` error types migrate separately".
- [ ] **Step 4:** Run `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build --workspace --locked`. Expected: clean. Manually run the API with an invalid `PP_DATABASE_URL` and confirm output shows the message and no credentials.
- [ ] **Step 5:** Commit `feat: report startup errors with color-eyre (#103)`.

### Task 4: Review and delivery

- [ ] **Step 1:** Fresh Opus reviewer against design and Review Focus; fix material findings.
- [ ] **Step 2:** Open PR (app token, assign @alekece, `Closes #103`); confirm CI green.
- [ ] **Step 3:** Remove this plan before requesting merge approval.
