# Credential-safe database error causes

Status: Approved in conversation on 2026-10-31.
Issue: [#106](https://github.com/alekece/photo-proofing/issues/106).
Follows #103 / #104 (`thiserror` migration), which dropped the cause of `ApiError::Connection` and `ApiError::Migration`.

## Intent

An operator who sees "database connection failed" must be able to tell why (refused, unknown host, authentication, TLS, bad configuration), without the database password or URL ever reaching a report, log, or span. `sqlx` types stop leaking out of `pp-database`.

## Goals and non-goals

- Goals: a `pp-database` error type that carries a sanitized cause; its use for connection, migration, and every repository error (including `GalleryError::Database`); a regression test proving a fake password never appears; a visible "connection refused" instead of a 30-second "pool timed out".
- Non-goals: changing HTTP responses (all database failures stay `unavailable`); `Database::new(PgPool)` and the `PgPool` re-export used by `#[sqlx::test]` in `api` tests (revisit separately); the `image-worker` crate.
- Anti-goal: any variant that can echo a URL component, even if it usually doesn't.

## Findings (probed against `sqlx` 0.8.6 with the password `s3cretpw`)

- Only `Configuration` errors echo URL content (`unknown value "s3cretpw" for ssl_mode`); `invalid port number` and `relative URL without a base` don't, but the variant as a whole cannot be trusted.
- I/O (DNS), TLS, and server errors (`password authentication failed for user "user"`) never contained the password.
- Migrations run on an established pool, so their errors cannot contain the URL.
- A refused connection is retried by the pool for 30 seconds and surfaces only as `PoolTimedOut`.
- `api` never inspects `sqlx` variants; it maps every `SqlError` to `ErrorResponse::unavailable()`.

## Approaches

1. **Allowlist by `sqlx` variant**, keeping `sqlx::Error` as the source for safe variants. Small, but `sqlx::Error` stays public. Rejected.
2. **Own error type in `pp-database` (selected by the user).** It converts from `sqlx::Error`, classifies it, and keeps only sanitized detail. The leak rule lives in one place and `sqlx` is hidden.
3. Scrubbing the password from message text. Fragile. Rejected.

## Design

- New `DatabaseError` in `pp-database` (`thiserror`, layer-only `Display`). Kinds: unreachable (carries the `io::Error`), authentication failed, TLS, configuration, timeout, migration, and other. `From<sqlx::Error>` is the single place that classifies.
- `configuration` carries no detail: a fixed message only. Every other kind may carry its cause as a source or message, because the probe showed those never contain URL content.
- Server errors keep the SQLSTATE code and message. The server's `detail` (which can hold row values) is included only under `cfg(debug_assertions)`, so release builds never carry it (user decision).
- Migration errors keep their message text.
- `Database` gains `connect_lazy` (URL and pool size in, `DatabaseError` out; parses the options and builds the pool without I/O), `ensure_reachable`, and `migrate`, so `Api::init` stops using `PgPoolOptions` and `MIGRATOR` directly. `ensure_reachable` makes one standalone connection attempt from the stored options and closes it, so a refused connection is reported immediately and not as a 30-second pool timeout (user decision: keep the attempt, expose it as a named method). `Api::init` calls it before `migrate`.
- Repositories return `DatabaseError` instead of `sqlx::Error`; `GalleryError::Database` wraps `DatabaseError` as its source. The `SqlError` re-export is removed, and `api`'s `From<SqlError> for ErrorResponse` becomes `From<DatabaseError>`.
- `ApiError::Connection` and `ApiError::Migration` carry `DatabaseError` as `#[source]` and still get no `From` impl. Their doc comment about credentials is rewritten to explain the sanitization.
- `AGENTS.md`: no change needed; this implements its existing rule against exposing dependency types.

## Constraints and checks

- Regression test: classify real-shaped `sqlx` errors (a configuration error echoing `s3cretpw`, an I/O error, a TLS error) and assert the `Display` and `source()` chain never contain it. DB-backed tests continue to run only in CI.
- Verify manually that an unreachable host, a refused port, and a bad `sslmode` print a useful cause with no password.
- Usual checks: `cargo +nightly fmt --all`, clippy with `-D warnings`, `cargo test --workspace --locked`, `cargo deny`.

## Left to the builder

The exact kind names and fields, method signatures (the shape above is my proposal), the pool size and timeout values (keep today's 10 connections), and whether `Other` carries a message or only a code. Call I made that you can override: leaving `Database::new(PgPool)` alone.
