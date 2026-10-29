# Account Setup and Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver #63: persistent one-time account setup, email/password login, protected dashboard, and revocable sliding sessions.

**Architecture:** Keep typed operator settings and identity types in core, SQLx migrations and transactional persistence in database, and password/token/cookie/request boundaries in API. SvelteKit checks auth server-side and uses same-origin cookie-authenticated API calls in the browser. No general session-store abstraction or extra service.

**Tech Stack:** Existing Rust/Actix, clap-config-fallback, SQLx/PostgreSQL, RustCrypto Argon2id/SHA-256 and OS-backed randomness; existing SvelteKit with Tailwind/shadcn-svelte components needed by auth forms. Confirm supported dependency versions before adding them and commit lockfiles.

**Spec:** docs/superpowers/specs/2026-10-29-account-sessions-design.md

**Issue:** https://github.com/alekece/photo-proofing/issues/63

## Global Constraints

- Isolated worktree .worktrees/63-account-sessions, branch feature/63-account-sessions, base 1a7db617f97f3efaea27b7eeabbe0ca60d7768f0. Leave main and unrelated untracked files untouched.
- HTTP trusted-LAN trial; keep localhost host bindings. No certificates, public exposure, or full LAN deployment work (#74).
- One account/workspace, no Remember me, one persistent cookie, default inactivity 30d, configurable human duration, sliding renewal on user activity only, no absolute maximum age.
- Import std::time::Duration as Duration, parse with humantime through existing config machinery. Core HumanDuration only if integration proves insufficient; no field-level serde(with) adapters or manual Rust environment loader.
- Config CLI > environment > TOML > defaults. New groups use sections; leave unrelated existing flat settings compatible.
- Token is 256 random bits, only SHA-256 digest persisted; no credential/token logs or JavaScript token storage. Logout revokes that session everywhere; unrelated sessions remain.
- Account creation is transactional and database-enforced under concurrency. All accepted data survives ordinary restarts/container recreation.
- No gallery/SSE implementation, account recovery/management, roles, bearer/refresh-token subsystem, session dashboard, or unrelated refactors.
- Use meaningful regression/integration tests; do not replace real browser/API/database evidence with mocked UI tests.
- Plan stays committed through review/corrections and is removed before final merge approval. Preserve approved design and lasting decisions/evidence. Never auto-merge.

## Review Focus

- Out-of-order renew/logout responses must not revive a revoked session; copied tokens fail subsequent authentication.
- Very short configured timeouts and fractional human durations must not silently produce zero/overflowing cookie lifetimes or renewal intervals.
- Prefetch, passive auth status, visibility changes, polling, and timers must not extend idle sessions; actual navigation/input must renew.
- Concurrent setup and service failures cannot produce orphan workspaces, duplicate accounts, misleading setup-open state, or successful logout without revocation.
- Browser/SSR cookies and Origin checks must agree through both native Vite proxy and Compose; database outages must not render cached protected HTML as valid.

## File map

- core: config.rs (with config/duration.rs and config/origin.rs validated types) plus config tests; auth.rs for AccountId, WorkspaceId and authenticated identity if shared by database/API. No secret-bearing Debug/Serialize types.
- database: lib.rs, auth.rs (with its tests), migrations/0001_accounts_sessions.sql; no generic repository trait.
- API: routes/auth.rs for handlers and AuthError, credentials.rs for session-token and password-hash types, state.rs for AppState (policy, limiter), server.rs for the Api init/run type; lib.rs mounts routes; tests/auth.rs. Preserve health tests.
- web: lib/auth.ts (nonsecret client types/calls/activity helper), lib/server/auth.ts (SSR status), routes/login/+page.server.ts and +page.svelte, dashboard/+page.server.ts and +page.svelte; focused unit/route tests and actual browser integration tests. app.css, layout and minimal shadcn component/config files only as needed.
- runtime/docs: Cargo manifests/lock, web package manifest/lock, config.example.toml, .env.example, docker-compose.yml, docker/web.Dockerfile only for matching existing Node 24 baseline, README, SPECIFICATIONS, MVP release, ADR 0002, earlier approved design supersession note, and #63 text.

## Task 1: Typed settings and human durations

**Interfaces:** SessionConfig { idle_timeout: Duration }; DatabaseConfig { url: String }; AuthConfig { app_origin: String }. CLI flattens groups but #[config(no_flatten)] keeps [session], [database], [auth]. Use PP_SESSION_IDLE_TIMEOUT / --session-idle-timeout, PP_DATABASE_URL / --database-url, PP_APP_ORIGIN / --app-origin. TOML names idle_timeout, url, app_origin within their sections. Frontend PP_API_INTERNAL_URL is separate Svelte runtime configuration.

- [ ] Write config tests: 30d default; TOML [session] idle_timeout="1h 30m" -> 5400s; CLI overrides environment/TOML; isolated subprocess environment overrides TOML; invalid/zero/negative/overflow inputs fail; new sections do not break flat host/port/log_level. Test origin validation rejects paths/credentials/non-HTTP(S) schemes.
- [ ] Run focused tests and observe new behavior fail before adding settings. Use the existing Duration value_parser/value_format support, not a custom deserializer. Enforce a minimum representable one-second session lifetime; reject subsecond lifetimes rather than silently truncating to zero. Validate checked conversion to cookie and database timestamp arithmetic at startup.
- [ ] Implement settings/defaults with synthetic native database URL and http://localhost:5173 origin; retain existing precedence. No Debug output of database URLs. Add sample sections and defaults to config.example.toml.
- [ ] Run cargo test -p pp-core --locked and nightly formatting. Expected: all config/core tests pass, including human duration round-trip/precedence. Commit task.

## Task 2: Transactional account and session persistence

**Interfaces:** AuthStore::new(PgPool); setup_available(); create_account(email, password_hash, token_hash, expires_at) -> Identity or SetupClosed; account_by_email(email); create_session(account_id, token_hash, expires_at, replaced_hash); authenticate(token_hash, now) -> Option<Identity>; renew(token_hash, now, expires_at) -> Option<Identity>; revoke(token_hash); cleanup_expired(now, limit). Database errors remain distinguishable from missing/expired sessions. Identity contains AccountId and WorkspaceId only; time arguments use one compatible timestamp representation consistently.

- [ ] Write SQLx integration tests with isolated migrated databases: exactly one concurrent setup succeeds; transaction rollback leaves no partial workspace/account/session; normalized email uniqueness; account/workspace FK integrity; raw token absent; expiry exactly at now is denied; renewal extends only valid sessions; logout/copy denied; renew/logout races never insert sessions; replacing a browser's session leaves another independent login valid.
- [ ] Add SQLx PostgreSQL/runtime/migrate/uuid/time support and migration. Enforce singleton account with a database constraint; create workspace/account/session together in one transaction. Store token digest as 32 bytes with lookup/expiry indexes. Use runtime-bound SQL or checked offline metadata so ordinary compilation needs no live database.
- [ ] Implement AuthStore using SQLx query binding, conditional UPDATE renewal (no UPSERT), transactional session replacement, and bounded expiry cleanup (default batch 100). Reuse existing PostgreSQL CI service; test databases are disposable and must have required creation privileges. Never use a real application database for destructive fixtures.
- [ ] Run DATABASE_URL=<throwaway-PostgreSQL-admin-url> cargo test -p pp-database --locked. Expected: integration tests pass; migration applies to empty databases and is safe to rerun through the migrator. Commit task.

## Task 3: Authentication endpoints and revocable cookies

**Interfaces:** configure_routes retains public /api/v1/health and registers /api/v1/auth/{status,setup,login,logout,renew}. AppState owns AuthStore, validated config, and bounded hashing/attempt controls. status JSON: {setup_available: bool, identity: {account_id,workspace_id}|null}; setup/login return identity JSON and Set-Cookie, renew returns 204 and refreshed cookie, logout returns 204 and removal cookie. Errors use safe JSON messages with 400 validation, 401 authentication, 403 origin, 409 setup closed, 413 size, 415 content type, 429 attempts, and 503 unavailable dependencies.

- [ ] Write API tests first for the approved setup/login/status/renew/logout contract, protected renewal denial, cookie attributes, generic bad-login behavior, no-store, original-cookie replacement, exact Origin checks including missing/null/foreign, malformed JSON, secret-free errors, and no renewal on GET status. Include database outage, setup race, logout failure, and expired/copy/race cases from Review Focus.
- [ ] Verify crate compatibility and add established password hashing, randomness, digest, token encoding, and email validation dependencies. Normalize email, accept 12–128 Unicode password characters without trimming; bound bodies to 4 KiB and email to 254 bytes. Encode the 32 random token bytes with URL-safe unpadded base64; reject noncanonical/oversized cookies. Token cookie name pp_session, host-only Path=/, HttpOnly, SameSite=Lax, persistent Max-Age, Secure=false for accepted HTTP trial. Never derive Debug for tokens/password inputs.
- [ ] Hash with Argon2id using documented parameters, off async workers. Use at most two concurrent hash jobs; shared bounded limiter permits 10 credential attempts per peer IP per minute, caps tracked peers at 1024, prunes stale entries, and rejects when full. No trust in forwarded client IP headers; document shared proxy rate limit. Tests inject a clock and smaller limits where appropriate; unknown-account login performs comparable verification work.
- [ ] Implement transactional successful setup/login and fresh session issuance, idempotent successful revoke/logout, atomic conditional renew, and safe status. Use constant-time library password verification. Reject bad origin/content type before work. Session cookie is proof, not a bearer header. A late renew response may leave a now-invalid cookie but cannot revive the server session.
- [ ] Wire pool/config into API startup and migrate before listening; preserve configure_routes use by health tests with health independent of auth state. Make failures safe without printing DB URLs. Add Compose API DB URL/origin/timeout variables and update .env.example; preserve localhost bindings and worker behavior.
- [ ] Run cargo test --workspace --locked with throwaway DATABASE_URL and cargo clippy --workspace --all-targets --locked -- -D warnings. Expected: current and new Rust tests pass, no lint warnings. Commit task.

## Task 4: Login UI, dashboard guard, and activity renewal

**Interfaces:** getAuthStatus(fetch, incoming_cookie?) -> status or typed service failure; SSR helper fetches PP_API_INTERNAL_URL (native default http://127.0.0.1:8080), forwards Cookie, never sends it to a browser-supplied target, and sets no-store. Browser auth calls use /api/v1/auth, same-origin JSON POST, no token storage. Renewal interval is min(5 minutes, idle_timeout/2); expose only nonsecret timeout/renewal metadata needed by frontend.

- [ ] Write route/helper/component tests first: empty installation setup; existing account login; authenticated /login redirect; dashboard SSR redirect before HTML for invalid session; failed dependencies -> useful 503 rather than setup/login success; successful/invalid forms; no Remember me; logout waits for revocation; 401 versus transient failure handling. Use clock-controlled interaction tests for short timeout and coalescing; no renew on timer/prefetch/passive status alone.
- [ ] Resolve the broken Corepack default using explicit pnpm 10.13.1 without global configuration edits. Install the existing lockfile first; adopt compatible Tailwind Vite integration and only shadcn Button/Input/Label components. Keep existing mock gallery and its tests unchanged; inspect generated files and avoid unrelated catalogs or aliases.
- [ ] Implement SSR guards and minimal accessible setup/login forms plus dashboard logout. Successful submit redirects to dashboard; setup conflict refreshes the form mode. Validate server-side as authoritative, prevent duplicate submit, support password managers and meaningful status/error announcements. Protect page/API caching.
- [ ] Implement one activity listener tied to the authenticated page lifetime: renew on real navigation/keyboard/pointer interaction, coalesce using the timeout-aware interval, remove listeners when leaving; no autonomous renewal timers or SSE renewal. Failed renewals do not spin; 401 returns to login and transient failures retain credentials. Login/setup begin with known last-renewed time.
- [ ] Set PP_API_INTERNAL_URL=http://api:8080 for the Compose frontend; configured public API origin matches the external proxy port. Align Docker web Node image with existing Node 24 tooling where required. No production-adapter or LAN exposure changes.
- [ ] Run explicit pnpm 10.13.1 --dir web check, test, build. Expected: all checks pass, keyboard forms work, no axe violations, existing mock route/viewer regressions absent. Commit task.

## Task 5: Real acceptance, aligned documentation, and independent review

**Interfaces:** consumes completed backend/frontend; produces real verification evidence and a PR referencing #63. Add narrowly scoped web browser tests under web/e2e/auth.spec.ts and its runner configuration if no equivalent existing harness exists; do not build a general E2E platform.

- [ ] Before fixing integration failures, write/run a failing real browser scenario against disposable PostgreSQL: initial setup, dashboard, browser restart using persisted cookie, normal login, expired session, activity renewal, logout, copied-cookie replay denied, and independent second session unaffected. Exercise native and Compose proxy paths, correct/wrong origins, unavailable API/database, and short configured duration. Restart API/recreate containers preserving database volume and demonstrate retained account/session; never delete real or unrelated volumes.
- [ ] Run required integration checks using synthetic credentials. Capture failures truthfully and fix within approved scope; no claims of E2E from mocked fetch or component tests. Keep UI screenshots/evidence out of product source unless needed as enduring docs. Verify 320px mobile and desktop keyboard layouts; full separate-device LAN trial remains #74.
- [ ] Update FR-01/AC-08 and the affected journey/MVP acceptance text from Remember me to persistent configurable sliding inactivity with no absolute cap and revocable logout. Add a supersession note linking this design to the prior product-restart record without rewriting history. Record ADR 0002 for the chosen session model and HTTP trial tradeoff; update README native/Compose setup, database requirement, session/origin config, credential limitations, and current status without claiming gallery completion.
- [ ] Update #63 to match the already approved session scope, preserving other acceptance criteria and original motivation. No unrelated issues, requirement changes, milestones, or deployment tooling.
- [ ] Run cargo +nightly fmt --all --check; cargo clippy --workspace --all-targets --locked -- -D warnings; DATABASE_URL=<throwaway> cargo test --workspace --locked; explicit pnpm 10.13.1 web check/test/build and auth browser tests; docker compose config --quiet; git diff --check; local doc links. Expected: passing results or explicit evidenced environment blockers, never hidden failures.
- [ ] Self-review plan/design coverage and full branch diff. Request one fresh-context independent review of correctness, security, and approved-design compliance, including Review Focus. Address material findings with regression coverage and rerun affected checks. Preserve findings, rulings, and evidence in permanent design/ADR or PR. Commit corrections.
- [ ] Push feature/63-account-sessions and create PR referencing #63 with current results and limitations. Keep this plan for PR review/corrections. Stop for user review, do not poll CI or auto-merge; remove the plan before final merge approval.

## Planning self-review

All approved behavior maps to Tasks 1–5; no independent feature subsystem is added. Review Focus maps to database/API race and failure tests (Tasks 2–3), duration/renewal tests (Tasks 1/4), and browser/proxy acceptance (Task 5). Shared identity/time/store interfaces are produced by Tasks 1–2 before consumers; HTTP behavior is fixed before UI. Exact supported dependency versions and generated UI file details are compatibility choices, not new product decisions. No implementation has started.
