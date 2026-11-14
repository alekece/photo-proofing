# Staging Mailpit Catch-All SMTP

Status: Approved.

Related: [#366](https://github.com/alekece/photo-proofing/issues/366), [#348](https://github.com/alekece/photo-proofing/issues/348), [Dashboard staging deployment](2026-11-09-staging-dashboard-deployment-design.md).

## Intent

Staging runs on a private LAN without a public domain (no SPF/DKIM), so external domain-verified SMTP relays cannot be used out of the box. Adding a catch-all `mailpit` service (`axllent/mailpit:v1`) to `deploy/compose.yml` lets gallery invitations and rendered Warm Paper emails be tested end-to-end on the LAN without sending real external mail or touching production application images.

## Goals, Non-goals, and Anti-goals

- **Goals:** Out-of-the-box staging email delivery into Mailpit with its web UI exposed directly on `${PP_BIND_ADDRESS}:${PP_MAILPIT_PORT:-8025}:8025` (no Caddy routing); keep SMTP `:1025` internal to the Compose network; bound stored messages in memory (`MP_MAX_MESSAGES=500`).
- **Non-goals:** Local `docker-compose.yml` changes, production SMTP relay configuration, or Caddy reverse-proxying for Mailpit.
- **Anti-goals:** Binding `0.0.0.0` on the host, publishing SMTP `:1025` outside the Compose network, or committing real addresses or credentials.

## Approaches

1. **Chosen — Direct port on `PP_BIND_ADDRESS`:** Publish `${PP_BIND_ADDRESS}:${PP_MAILPIT_PORT:-8025}:8025` directly on `mailpit` with optional `MP_UI_AUTH:` pass-through from the secret manager, and default `x-app-config` SMTP variables to `mailpit:1025`. Minimal moving parts, no DNS or Caddy changes.
2. **Not chosen — Caddy `/mailpit/` subpath or dedicated vhost:** Adds unnecessary reverse-proxy or private DNS setup for a LAN-only catch-all inbox.

## Design

- **`mailpit` service (`deploy/compose.yml`):**
  - `image: axllent/mailpit:v1` with `*runtime` logging/restart policy.
  - `environment:` `MP_MAX_MESSAGES: "500"` and bare `MP_UI_AUTH:` (forwarded only when set in the secret manager, e.g. `user:password`, to password-protect the web UI).
  - `ports:` `"${PP_BIND_ADDRESS:?Select a loopback or LAN address}:${PP_MAILPIT_PORT:-8025}:8025"`. Port `1025` stays unexposed outside the Compose network.
  - `healthcheck:` `["CMD", "/mailpit", "readyz"]` (`interval: 5s`, `timeout: 3s`, `retries: 10`).
- **Default staging SMTP wiring (`x-app-config`):**
  - `PP_SMTP_HOST: ${PP_SMTP_HOST:-mailpit}`
  - `PP_SMTP_PORT: ${PP_SMTP_PORT:-1025}`
  - `PP_SMTP_TLS: ${PP_SMTP_TLS:-none}`
  - `PP_SMTP_FROM_EMAIL: ${PP_SMTP_FROM_EMAIL:-noreply@proofing.example}`
  - Keep `PP_SMTP_USERNAME:`, `PP_SMTP_PASSWORD:`, `PP_SMTP_FROM_NAME:`, and `PP_SMTP_MAX_RECIPIENTS_PER_SEND:` as bare `null` pass-throughs so external SMTP testing remains a pure stack/secret-manager override.
- **Documentation (`README.md`):** Document `http://<PP_BIND_ADDRESS>:8025`, optional `PP_MAILPIT_PORT` / `MP_UI_AUTH`, and overriding `PP_SMTP_*` when testing an external relay.
