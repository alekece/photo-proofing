# ADR 0002 — Revocable PostgreSQL sessions

Status: Accepted through the approved [account-session design](../superpowers/specs/2026-10-29-account-sessions-design.md), issue [#63](https://github.com/alekece/photo-proofing/issues/63).

## Context

The photographer needs one-time account setup and persistent sign-in with activity-driven inactivity expiry. Logout must invalidate copied credentials as well as the current browser cookie. PostgreSQL is already the authoritative persistent store. Local development defaults to HTTP; an HTTPS deployment must be able to enable secure session cookies.

## Decision

Use opaque 256-bit random session tokens in host-only HttpOnly cookies. Store only their SHA-256 digests, account references, and expiry in PostgreSQL. Authenticate against live, nonexpired records; renew only existing records and delete them on logout. Use one persistent-session policy, no Remember me checkbox, a configurable human-readable inactivity timeout defaulting to 30d, and no absolute session lifetime.

Use RustCrypto Argon2id for password hashing, same-origin state-changing requests, and bounded credential processing. Do not add a generic session-store framework, a separate cache, bearer-token browser storage, or a refresh-token subsystem.

The session `secure_cookie` setting is configurable through TOML, environment and CLI input. It defaults to `false` for local HTTP and must be enabled when the app is served over HTTPS.

## Alternatives and consequences

A self-contained protected cookie avoids a session lookup but cannot revoke a copied token without additional server state. Sliding renewal could let a copied token keep working indefinitely. Actix session middleware with a PostgreSQL adapter would add a general storage contract without simplifying this typed session policy. Both were considered; focused PostgreSQL sessions were chosen.

A database lookup is required for authentication and a write for coalesced activity renewal. Database outages deny protected access; accounts and sessions survive normal recreation of API containers. Concurrent renewal cannot resurrect a session deleted by logout. Requests already authorized before logout may finish; subsequent authorization fails. Logout affects that session, not independent logins.

HTTP exposes passwords and cookies to network interception. This trial decision does not make the system suitable for real client credentials or public deployment. The `Secure` cookie flag is off by default and must be enabled for HTTPS; the setting does not itself configure TLS or make deployment safe for public access. In-process attempt limiting shares a proxy peer address and resets on API restart; distributed abuse controls are outside this slice.

Known limits accepted for the trusted-LAN trial, to be revisited before public exposure: the first visitor to reach setup becomes the owner; setup and login refuse requests beyond two concurrent password hashes without counting them as failures, and `POST /setup` hashes before checking that setup is still open, so a hostile peer can keep the hash slots busy and delay sign-in; the attempt limiter refuses new peers once it tracks 1024. Request throttling belongs at an API gateway in production.
