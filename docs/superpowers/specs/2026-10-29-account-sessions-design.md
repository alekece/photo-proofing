# Photographer account setup and persistent sessions

Status: Approved in conversation on 2026-10-29, including the subsequent human-readable Duration and sectioned-configuration refinements. Implementation follows a written plan.
Issue: [#63](https://github.com/alekece/photo-proofing/issues/63).

## Intent and user outcome

On a new installation, a photographer opens /login, creates the only account, and enters their private dashboard. On later visits they sign in with email/password when necessary. They stay signed in across browser restarts until the configured inactivity timeout or explicit logout. This is the first persistent end-to-end MVP slice; galleries and media remain outside its scope.

## Decisions approved in conversation

- Keep HTTP for the trusted-LAN MVP. Defer certificates, HTTPS deployment, and public-internet access. Document that HTTP exposes passwords and cookies in transit; use replaceable trial credentials.
- Remove Remember me. Use one persistent session policy with a configurable inactivity timeout, proposed default 30 days and approved during the discussion.
- Renew on authenticated user activity. Background polling, SSE connections, and keep-alives do not renew sessions.
- No absolute session lifetime: continued activity can keep a session alive indefinitely.
- Use PostgreSQL-backed sessions so logout invalidates every copy of that particular session token, rather than merely clearing one browser cookie. Other independent login sessions are unaffected.

These decisions supersede this slice's earlier Remember me/two-timeout requirements in FR-01, AC-08, MVP acceptance, and issue #63. Updating those records is part of this slice; broader product/release scope is unchanged. The previous product-restart design remains a historical record, with its Remember me decision superseded by this design once approved.

## Approaches considered

1. A focused opaque-token session stored in PostgreSQL: selected. It supports immediate revocation, persistence, and exact renewal behavior without another service or a general-purpose session adapter.
2. Actix session middleware with a custom PostgreSQL store: useful generic cookie/session machinery, but requires fitting a general session-state contract and adapter to a small typed session record. Not selected for this slice.
3. A protected self-contained cookie without session records: simpler storage, but deleting the cookie cannot invalidate a copied token; sliding renewal could let its holder keep access indefinitely. Discussed and rejected in favor of server-side revocation.

Use established cryptographic libraries and Actix cookie handling. Do not write hashing algorithms, encryption schemes, or a custom token format for signed claims.

## Scope and constraints

Deliver setup/login/logout UI, protected dashboard access, account/workspace/session persistence, migrations, operator configuration, Compose wiring for the database, tests, and current setup documentation. Adopt the already agreed Tailwind/shadcn-svelte choices only as needed for these screens, verifying version compatibility.

No gallery features, media or SSE implementation, Google sign-in, password recovery/change interface, account management, roles/collaborators, session-management screen, bearer-token frontend storage, refresh-token subsystem, Redis, or HTTPS provisioning. Preserve the existing health endpoint and mock gallery behavior; public mock pages must not be mistaken for protected account data.

Do not implement in the main checkout. Use .worktrees/63-account-sessions on feature/63-account-sessions. Preserve unrelated files. SQLx/PostgreSQL and the existing configuration precedence remain the baseline.

## Browser and API behavior

/login determines whether initial setup is available and shows either an email/password account-creation form or a sign-in form. No Remember me control. Successful setup or login establishes a fresh session and opens /dashboard. Already authenticated visitors go directly to the dashboard. Setup availability is not a security boundary: the database enforces one initial account under concurrency.

The dashboard remains a minimal authenticated workspace shell with logout. Unauthenticated/expired requests redirect to /login before protected HTML is returned. Future photographer routes must use the same account/workspace authentication boundary. Protected APIs return 401 for missing, malformed, expired, or revoked sessions; unavailable database dependencies return a safe service failure rather than being treated as successful authentication or empty setup state.

Proposed API surface, left to the builder to refine without changing behavior:

- GET /api/v1/auth/status: whether setup is available and the current authenticated identity, without renewing the session; no passwords, hashes, or session tokens in JSON.
- POST /api/v1/auth/setup and POST /api/v1/auth/login: bounded email/password input, create a fresh session, and set its cookie. Setup conflicts return a useful setup-closed response. Login failures use a generic invalid-credentials response.
- POST /api/v1/auth/logout: revoke the presented session and expire its cookie; safe and idempotent for an already missing/invalid session. A database failure must not falsely claim successful revocation.
- POST /api/v1/auth/renew: authenticate the current session and extend it. Frontend invokes this only for navigation or user interaction, coalescing interaction-driven renewals with an interval of the smaller of five minutes or half the configured inactivity timeout, so short configured timeouts still work during active use. No timer alone renews a session. Login/setup establish initial expiry immediately. Expired sessions cannot be resurrected.

Browser calls use same-origin /api URLs and HttpOnly cookies, not JavaScript-managed bearer tokens or localStorage. SvelteKit server-side authentication checks explicitly forward the incoming cookie to the configured internal API and never renew during passive status checks. User navigation initiates renewal through the browser so Set-Cookie reaches it. Logout reaches the API before redirecting, and revoked credentials cannot load protected content afterward. Auth responses and protected pages use no-store caching.

The frontend handles 401 during use by returning to login with a useful expired-session message. It must not clear sessions on transient network/service errors or show stale protected pages as successfully authenticated after a failed check. Form controls support password-manager autocomplete, labels, keyboard interaction, visible focus, pending submission, and accessible error messages. Prevent accidental duplicate submissions without relying on the UI for concurrency correctness.

## Session and credential model

Persist account, workspace, and sessions with SQLx migrations. One setup transaction creates the workspace, account, and initial session, with a database-enforced singleton account invariant. Concurrent setup cannot leave extra workspaces/accounts or partial setup. All later domain ownership is workspace-based; do not introduce a collaboration framework for this slice.

Author proposals for credential validation: normalize email by trimming and lowercasing, validate a conventional email address with an established validator, and store it uniquely. Passwords are not trimmed or transformed; accept 12–128 Unicode characters with a bounded request body, allow spaces and password-manager paste, and hash using RustCrypto Argon2id with random per-password salts. Perform expensive hashing off async request workers with bounded concurrency. Exact supported crate versions, limits for bytes versus characters, and hashing parameters must be verified during planning; avoid custom email parsers.

On successful login, generate a cryptographically random 256-bit opaque token. Store only a SHA-256 digest of that token with session/account identity and server expiry in PostgreSQL. The raw token appears only in the HttpOnly cookie; never in logs, URLs, JSON, or database records. The cookie is host-only, Path=/, SameSite=Lax, and persistent with Max-Age matching the remaining lifetime. Secure is false for the explicitly accepted HTTP trial; document HTTPS/Secure as a later deployment change, not implemented here.

Authentication validates token syntax/size and digest, joins the associated account/workspace, and requires server expiry strictly later than the current time. An expired cookie is not sufficient protection: enforce expiry server-side. Login creates an independent session; revoke a valid session presented by the same browser when replacing it, without revoking other devices. Failed login must not replace a valid session or disclose account existence.

Renewal atomically extends only a still-valid, nonrevoked record and returns the refreshed cookie. Logout atomically revokes/deletes the record, then expires the cookie with matching attributes. A concurrent renewal cannot reinsert/revive a deleted session; requests already authorized before logout may finish, but subsequent checks fail. Renewal uses a single server-side clock and deterministic clock control in tests. Cookie persistence survives API/container restart because session validity is in PostgreSQL; no ephemeral signing secret is required for the opaque cookie.

Expired records are invalid immediately and are cleaned up in bounded batches during login/renewal rather than requiring a new worker service. Detailed indexes, queries, and cleanup batch size are routine builder decisions. Do not log passwords, token values/digests, cookies, or credential request bodies.

## Configuration and request boundaries

Use the existing CLI > environment > optional TOML > default precedence for API operator settings. Add database connection settings, internal API target for SvelteKit server checks, the externally visible application origin, and one session inactivity duration. Proposed names: PP_DATABASE_URL, PP_API_INTERNAL_URL, PP_APP_ORIGIN, and PP_SESSION_IDLE_TIMEOUT (CLI --session-idle-timeout, TOML [session] with idle_timeout). Group related new settings in cohesive TOML sections using the existing config library's section support; keep environment names and CLI flags explicit. Do not migrate unrelated existing server/log settings just for consistency. Accept human-readable duration strings consistently, for example "30d", "12h", or "1h 30m", defaulting to "30d". Import the duration type as Duration. Prefer std::time::Duration with humantime::parse_duration as the Clap value_parser: the installed clap-config-fallback derive forwards explicit parsers into TOML deserialization as well. Verify CLI/environment/TOML behavior with focused tests during implementation. If the existing parsing path proves insufficient, introduce one reusable HumanDuration type in core with parsing/serialization implemented on the type, rather than field-level serde(with) adapters. Reuse ConfigArgs/ConfigParser for precedence rather than adding manual environment reads or another configuration loader. Reject zero, negative, malformed, or values beyond representable cookie/timestamp arithmetic; do not impose an absolute session-age limit. Changes apply to new sessions and subsequent renewals, not retroactively by rewriting every session row.

Native defaults target the documented Vite origin and local API; Compose sets the proxy origin and internal API service URL. Keep example database credentials synthetic, .env/config files ignored, and connection strings out of logs. API startup connects to PostgreSQL and applies migrations before serving auth endpoints; DB failure prevents startup rather than opening setup on an unknown state. Health remains an unauthenticated non-sensitive endpoint.

All auth mutations use POST with bounded JSON bodies and an Origin exactly matching the configured application origin, rejecting absent/foreign/null origins. Do not enable cross-origin credential access. Check this on setup/login/logout/renewal, not just existing authenticated actions. Never trust arbitrary forwarded headers to establish the allowed origin. The configured origin is an operator setting, not derived from the incoming Host header. SameSite and HttpOnly supplement these checks rather than replacing them. No state changes on GET, including session renewal.

Bound credential request size, expensive hash concurrency, and login/setup attempt rate with a small bounded in-process limiter. The implementation plan selects conservative defaults and tests them. Restart-persistent abuse monitoring and multi-proxy IP attribution are not introduced; document that the current reverse proxy may share one peer address and do not trust arbitrary X-Forwarded-For values. API/database errors expose no secrets. Keep API and database host bindings local as today; no LAN-wide port exposure changes are required for this slice. Full LAN deployment acceptance belongs to #74.

## Verification and delivery

- Database integration tests against a throwaway PostgreSQL instance: blank migrations, concurrent setup, rollback, persisted account/workspace/session, normalized credentials, correct/incorrect password, invalid tokens, expiry boundaries, renewal, logout/copied-token denial, independent sessions, and renewal/logout races.
- API tests: cookie attributes, no raw token/hash leakage, exact origin and content-type enforcement, bounded bodies/rate/concurrency, protected-route denial, generic sign-in errors, safe database failures, and no-store responses. Preserve health tests.
- Frontend tests: setup versus login, successful/failed submissions, dashboard guard, logout, expired/service-failure behavior, activity-only renewal, no Remember me, keyboard/forms/accessibility, and browser persistence behavior through appropriate integration coverage.
- Demonstrate the actual browser -> API -> PostgreSQL setup/login/dashboard/logout path with synthetic credentials, including copied-cookie rejection after logout and persistence across normal process/container recreation. Do not call mocked checks end-to-end evidence. Report what cannot be exercised.
- Run nightly formatting, relevant Rust Clippy/tests, frontend check/tests/build, migration checks, and Compose config validation. Verify credential defaults/configuration precedence and updated README instructions. No weakened tests.
- Update FR-01/AC-08, MVP setup/session acceptance, and issue #63 to the agreed single-policy contract. Preserve this design and link the earlier superseded Remember me decision; create a concise ADR for the lasting server-side session choice if implementation confirms it.
- After explicit approval of this written design, invoke Superpowers writing-plans. Implementation follows the approved design/plan in the isolated worktree, then a fresh independent review checks correctness and design compliance. Open a PR referencing #63; retain the working plan through review/corrections and remove it before final merge approval. Never merge automatically.

## Author proposals and remaining builder decisions

The exact API spelling, 12–128-character password validation proposal, timeout-aware frontend renewal coalescing, expired-session cleanup mechanism, and credential abuse limits are author choices, not earlier user decisions; approval of this document accepts those product defaults. The builder may refine routine interfaces, module layout, crate versions, query/index details, and numeric resource limits while preserving the contracts above. If compatibility or implementation evidence requires a consequential scope/security change, return for design discussion rather than silently changing behavior.

## Builder check

A fresh independent read-only agent reviewed this draft for material gaps before implementation planning and reported “nothing important left.” No product implementation or application tests were performed during design.
