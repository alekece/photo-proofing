# Failed-attempts middleware Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace PR #87's `FailedGuess` response marker with a status-driven `Transform`/`Service` middleware that records failed attempts on `login` and `setup`.

**Architecture:** `FailedAttempts<F>` wraps a route, and after the inner service responds it calls `ApiState::record_failure(peer)` when the predicate accepts the status. The default predicate counts `4xx` except `429`. `ErrorResponse` and `AuthError` lose all guess knowledge.

**Tech Stack:** Rust, actix-web 4 (`Transform`/`Service`, as in `middlewares/origin.rs`), sqlx tests.

**Spec:** `docs/superpowers/specs/2026-10-31-failed-attempts-middleware-design.md` (issue #79, branch `refactor/79-credential-failure-middleware`, work in `.worktrees/79-credential-failure-middleware`).

## Global Constraints

- `F: Fn(StatusCode) -> bool + 'static`, held in an `Rc<F>`; no `Clone` bound.
- Default rule: every `4xx` except `429` counts; `5xx` never counts.
- Only `POST /auth/login` and `POST /auth/setup` are wrapped; `status`, `logout` and `renew` are not.
- `ErrorResponse` stays generic; no typed error is reintroduced.
- `Attempt` remains the `429` gate; `/auth` keeps the API-wide `JsonConfig`.
- Repo rules: `use` at module top, `module.rs` + `module/child.rs`, no unit-suffixed primitives, blank lines around control flow, document non-obvious items.
- Checks: `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`.

## Review Focus

- A `429` from the gate must not be recorded (a limited peer's window must not grow).
- A `5xx` (store down) on `login` must not lock out the peer.
- Repeated `401`s on `renew`/`status` (unwrapped) must never exhaust the login allowance.
- A successful sign-in (`200`) never counts.
- A closure capturing non-`Clone` state is accepted as the predicate.

---

### Task 1: `FailedAttempts` middleware

**Files:**
- Create: `crates/api/src/middlewares/failed_attempts.rs`
- Modify: `crates/api/src/middlewares.rs` (add `pub mod failed_attempts;`)

**Interfaces:**
- Produces:
  - `pub(crate) struct FailedAttempts<F = fn(StatusCode) -> bool>`
  - `impl FailedAttempts { pub fn new() -> Self }` and `impl Default` (default rule above)
  - `impl<F: Fn(StatusCode) -> bool + 'static> FailedAttempts<F> { pub fn counting(predicate: F) -> Self }`
  - `Transform<S, ServiceRequest>` and `Service<ServiceRequest>` impls shaped like `SameOrigin` (`Response = ServiceResponse<B>`, boxed `'static` future).

- [ ] **Step 1: Write failing tests** in a `#[cfg(test)] mod tests` of the new file. Build an `ApiState` over a lazy pool (`PgPoolOptions::connect_lazy`; the tests never touch the database) with `max_failures = 2`. If a lazy pool is not usable, use `#[sqlx::test]` like `routes/auth.rs`. Mount a test service returning a configurable status, wrapped by `FailedAttempts`. Tests:
  - `default_counts_client_errors_except_too_many_requests`: statuses `401`, `422`, `413`, `415`, `400` are each recorded (`allow_attempt(peer)` flips to false after two); `429`, `503` and `200` leave `allow_attempt` true no matter how many calls.
  - `custom_predicate_replaces_the_default`: `counting(|status| status == StatusCode::CONFLICT)` records `409` and ignores `401`.
  - `closure_may_capture_non_clone_state`: a predicate capturing a non-`Clone` value compiles and runs.

- [ ] **Step 2: Run and verify they fail**

Run: `cargo test -p pp-api failed_attempts --locked`
Expected: FAIL to compile (`FailedAttempts` undefined).

- [ ] **Step 3: Implement** the types above in `failed_attempts.rs` with a `//!` overview (what is counted, why `5xx`/`429` are excluded, why the predicate lives in an `Rc`). In `call`, read `ApiState` from `app_data` and the peer via `RequestExt::peer_ip` before calling the inner service; after the response, if the predicate accepts `response.status()`, call `state.record_failure(peer)`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p pp-api failed_attempts --locked`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/api/src/middlewares.rs crates/api/src/middlewares/failed_attempts.rs
git commit -m "feat: add status-driven failed-attempts middleware"
```

### Task 2: Switch `login`/`setup` and remove the marker

**Files:**
- Modify: `crates/api/src/routes/auth.rs` (scope wrapping, module docs, remove `is_guess` and marking in `AuthError::error_response`, tests)
- Modify: `crates/api/src/error.rs` (remove `guess` field, `FailedGuess` import, marking; plain `422` arm)
- Modify: `crates/api/src/extractors/attempt.rs` (doc link)
- Modify: `crates/api/src/middlewares.rs` (drop `failed_guess`)
- Delete: `crates/api/src/middlewares/failed_guess.rs`

**Interfaces:**
- Consumes: `FailedAttempts::new()` from Task 1.

- [ ] **Step 1: Update tests in `routes/auth.rs`.**
  - Rework `requests_have_bounded_bodies_and_attempts`: the `415` and `413` now count, so the loop of invalid bodies expects `422` for 8 attempts, then `429`.
  - Add `other_routes_do_not_use_the_attempt_limit`: 10 `401`s against `/auth/renew` (no session), then a `login` is still `401`, not `429`.
  - Keep `wrong_logins_are_limited_per_peer` and `successful_logins_do_not_use_the_attempt_limit` unchanged.

- [ ] **Step 2: Run and verify the changed tests fail**

Run: `cargo test -p pp-api routes::auth --locked`
Expected: FAIL (`415`/`413` not yet counted by the new wiring).

- [ ] **Step 3: Implement.** In `configure`, remove the scope-level `.wrap(from_fn(record_failed_guesses))` and put `login` and `setup` each behind `.wrap(FailedAttempts::new())` (resource-level, e.g. `web::resource("/login").wrap(...).route(web::post().to(login))`, or a nested scope), dropping the `#[post]` attributes accordingly. Delete `failed_guess.rs`, remove the `guess` field and marking from `ErrorResponse`, and `is_guess` plus its use in `AuthError`. Update module docs in `auth.rs` and `attempt.rs` to reference `FailedAttempts`.

- [ ] **Step 4: Run the full checks**

Run: `cargo +nightly fmt --all && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace --locked`
Expected: all pass.

- [ ] **Step 5: Update docs and PR text.** Update #79's acceptance criteria (`413`/`415`/syntax `400` now count) and PR #87's description using `GH_TOKEN=$(~/.agents/bin/gh-app-token) gh ...`. Check `docs/SPECIFICATIONS.md` and `README.md` for claims that size or content-type rejections do not count; do not change product requirements without approval.

- [ ] **Step 6: Commit**

```bash
git add -A crates docs
git commit -m "refactor: count failed attempts by status in a middleware"
```
