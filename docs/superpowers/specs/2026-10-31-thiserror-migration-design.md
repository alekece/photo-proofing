# Migrate error types to thiserror

Status: Approved in conversation on 2026-10-31.
Issue: [#103](https://github.com/alekece/photo-proofing/issues/103).
Follows #92 (the `thiserror` standard in `AGENTS.md`) and #94.

## Intent

Bring the 10 `derive_more::{Display, Error}` error types in `core`, `database`, and `api` in line with the `AGENTS.md` standard, and make sure each cause is shown exactly once when an error is reported.

## Goals and non-goals

- Goals: `thiserror` on every error type; layer-only `Display`; causes exposed through `source()`; startup failures printed with the full cause chain.
- Non-goals: changing HTTP responses (`ErrorResponse`); redesigning `GalleryError::Database(sqlx::Error)` (it exposes a dependency type, against the new `AGENTS.md` rule, but `api` matches on `SqlError` today; revisit separately); touching `AuthError` and `error.rs`, which already use `thiserror`.

## Approaches

1. **`thiserror` + `eyre`/`color-eyre` at the `api` `main` edge (selected by the user).** `main` returns `eyre::Result<()>`, and `?` turns `ApiError` into a `Report` that prints the chain. Library crates keep typed errors, which matches the policy that opaque errors live only at binary edges.
2. `thiserror` + the `display-error-chain` crate at logging sites. Small, but it covers sites that do not need it: the only other error log, `warn!(%error)` in `delete_expired_sessions`, logs a bare `sqlx::Error`.
3. `thiserror` + a local chain-printing helper. Rejected by the "prefer established crates" rule.

## Design

- Replace `#[derive(Debug, Display, Error)]` with `#[derive(Debug, Error)]` (importing `Error` from `thiserror`, per the import rules) and `#[display(..)]` with `#[error(..)]`. Add `thiserror` to `core` and `database` through `[workspace.dependencies]`; `derive_more` stays for `Deref`, `FromStr`, `From`, and newtype `Display`.
- Messages become layer-only: remove the embedded cause from variants that carry one (`database error: {_0}` becomes `database error`, `invalid duration: {_0}` becomes `invalid duration`, and so on), and mark the cause `#[source]` or `#[from]`. Leaf types such as `ParseEmailError` keep their messages unchanged.
- `ApiError::Connection` and `ApiError::Migration` keep no source: their causes can embed the database URL and its credentials. The `From` impls that exist today (via `derive_more::From`) keep existing through explicit `#[from]` on exactly the variants that have them now.
- `GalleryError::{LengthOverflow, InvalidCharacters}` keep their messages and gain no source.
- `crates/api/src/main.rs`: `main` returns `eyre::Result<()>`, calls `color_eyre::install()?` first, and drops the `IoError::other(error.to_string())` mapping. `eyre` and `color-eyre` are added to `[workspace.dependencies]` and used only by `api`. `image-worker` is unchanged (it has no domain errors).
- `AGENTS.md`: state that opaque error reports (`eyre`) belong only at a binary's `main`, and remove the "migrate separately" caveat.

## Constraints and checks

- Rendered messages in `ErrorResponse` and `AuthError` do not change; existing tests on messages must still pass.
- Add a test that an `ApiError::Connection` has no source, so no database URL can reach logs.
- `cargo deny --workspace check` must stay green; if the new dependencies need licenses outside the allow-list, add only what is required.
- Standard checks: nightly fmt, clippy with `-D warnings`, and `cargo test --workspace --locked` (database tests need PostgreSQL).

## Left to the builder

Exact reworded messages for each variant; whether `color-eyre` keeps its default features; the order of the mechanical edits per crate.
