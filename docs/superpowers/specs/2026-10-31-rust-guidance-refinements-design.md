# Rust guidance refinements

Status: Approved in conversation on 2026-10-31.
Issue: [#92](https://github.com/alekece/photo-proofing/issues/92).
Builds on [Repository Rust guidance](2026-10-29-rust-guidance-design.md), which stays the record of why `AGENTS.md` is the single policy file.

## Intent

Audit `AGENTS.md` against the themes of *Rust for Rustaceans* (types, interface design, error handling, testing, macros, async, concurrency, project structure) and add the missing rules that fit this project: a Rust application workspace (`publish = false`) with an Actix API, a separate image worker, and PostgreSQL. The book was used from knowledge of its chapters; no text is reproduced.

## Goals

- Record conventions the code already follows, so agents stop re-deriving them.
- Close gaps in error handling, async, concurrency, dependencies, secrets handling, macros, and documentation.
- Keep `AGENTS.md` short enough to read; one line per rule.

## Non-goals and anti-goals

- No `unsafe`, FFI, `no_std`, or Cargo-feature policy.
- No library-grade semver or public-API stability rules; the workspace is not published.
- No change to product scope. The separate image worker with a PostgreSQL queue (NF-02, NF-04, TC-03, ADR 0001) stays as specified. Merging the worker into the API was discussed and rejected for now.
- Do not add ceremony an application does not need (for example `Send + Sync` assertions everywhere).

## Approaches

1. **Extend `AGENTS.md` with application-scoped rules (selected).** Adopt only what fits a web app and worker; record existing practice. Cheap, matches the single-policy decision.
2. Adopt library-grade API discipline throughout. Rejected: the crates are unpublished, so most of it is cost without benefit. Revisit if `core` is ever published.
3. Put the rules in an ADR. Rejected: these are working conventions for agents, not an architectural decision.

## Rules to add (decided in conversation)

**Errors**
- Each crate or domain boundary defines typed error enums; callers branch on variants.
- Only `api` maps errors to HTTP status codes; domain crates never know about HTTP. Internal causes are never sent to clients, and every error response uses the shared `ErrorResponse` shape.
- Wrap infrastructure causes as `source`; do not expose `sqlx::Error` and similar types in public variants.
- Use `thiserror` for error types. Keep `derive_more` for other boilerplate (`Deref`, `FromStr`, `From`, `Display` on newtypes).
- `#[non_exhaustive]` only on types that are intentionally published.

**Async and concurrency**
- No blocking calls in async code; CPU-bound work (decode, resize) uses `spawn_blocking` or a bounded pool.
- Multi-step state changes (job claim, status update, publication) run in one database transaction, so a dropped future cannot leave partial state.
- Every spawned task has an owner: it is joined, or cancelled through a shared shutdown signal. No fire-and-forget.
- In-memory state holds only data that is cheap to lose (limiters, caches, live SSE subscribers). Anything that must survive a restart or be consistent lives in PostgreSQL.
- Prefer `std::sync::Mutex` for short critical sections and never hold a guard across `.await`; restructure with a channel or an owning task instead of reaching for `tokio::sync::Mutex`.
- Cross-task channels are bounded. A slow SSE consumer is lagged or dropped, never buffered without limit.

**Secrets and observability**
- A credential or bearer-secret newtype never derives `Debug`: omit it or implement a redacting one with a doc note. This includes share links, because the link grants access to client photos. `Token<N>` and `Password` already follow this.
- Secrets never appear in logs or spans; `#[instrument]` skips sensitive arguments. Every request or job span carries an identifier.
- A shared `Sensitive<T>` wrapper is deferred until a third distinct secret type needs it (my call; `Token<N>` covers today's cases).

**Interfaces and documentation**
- Apply `#[must_use]` as a principle to builders, guards, and results that are harmful to drop.
- New public items with non-obvious contracts get a short runnable doc example; `cargo test --workspace` already runs them.
- Common traits on public types where sound, except credentials.

**Dependencies**
- Prefer an established, well-tested, maintained crate over our own code, even for small needs. Write our own only when the crate is unmaintained, pulls heavy transitive dependencies, or the need is trivial.
- Declare dependencies in `[workspace.dependencies]` with minimal features.
- `cargo deny` (advisories, licenses, duplicates) gates CI.

**Macros**
- No `macro_rules!` by default; it needs a stated reason such as preventing drift between parallel definitions, plus a doc example.
- Declarative derives that generate companion types from one annotated definition (for example an entity and its changeset) are welcome. Look for an existing crate first.
- A custom proc-macro needs its own crate and an ADR explaining why no existing crate fits.

## Constraints

- Follow the repository workflow: issue before plan, isolated worktree, fresh independent reviewer, PR referencing the issue.
- `AGENTS.md` changes only; no source refactoring in this slice.

## Follow-ups (separate issues, not part of this change)

- Migrate `derive_more::{Display, Error}` error types in `core`, `database`, and `api` to `thiserror`.
- Add `cargo deny` configuration and a CI job.
- Candidates not yet decided: input-validation limits for image decoding, injected clock for deterministic expiry tests, forward-only migrations.

## Left to the builder

Exact wording and placement of each rule in `AGENTS.md`, keeping one line per rule and existing section order.
