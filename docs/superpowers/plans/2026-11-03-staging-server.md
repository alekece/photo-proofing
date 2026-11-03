# LAN Staging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prepare and deploy the published image set on the LAN staging server.

**Architecture:** Override local Compose with published images and isolated
internal ports. A small standard-library Python launcher reads the non-secret
TOML origin and Doppler-injected password, pins images to one Git tag resolution,
and runs Compose without displaying secrets.

**Tech Stack:** Docker Compose 2.24.4+, Python 3.11+, Doppler, GHCR, Caddy.

**Spec:** `docs/superpowers/specs/2026-11-03-staging-server-design.md`

**Issue:** https://github.com/alekece/photo-proofing/issues/137

## Global Constraints

- Only Caddy publishes ports, bound to the server's LAN address.
- Never build application images on the server.
- Keep secrets out of files and command output; never remove persistent volumes.
- Keep deployment manual and owner-controlled account bootstrap separate.

## Review Focus

- URL-special password characters must survive database URL encoding.
- Invalid/non-HTTPS origins must fail before contacting Docker.
- Local-development ports and environment defaults must not leak into staging.
- Failed pulls must leave running services unchanged.
- Missing secrets and unauthorized registry access must fail explicitly.

### Task 1: Reproducible staging deployment

**Files:** create `compose.staging.yml`, `docker/Caddyfile.staging`,
`docker/config.staging.example.toml`, `scripts/deploy-staging.py`,
`scripts/test_deploy_staging.py`; update `README.md` and `.github/workflows/ci.yml`
to run deployment checks for relevant changes.

**Interfaces:** launcher consumes a TOML path, LAN IPv4 address and
`POSTGRES_PASSWORD`; outputs one pinned Compose deployment and selected SHA.

- [ ] Write standard-library unit tests for origin validation, encoded passwords,
      environment derivation and pull-before-up failure behavior.
- [ ] Run `python3 -m unittest discover -s scripts -p 'test_*.py'` and observe failure.
- [ ] Implement the launcher and Compose/Caddy/config templates. Refuse missing
      inputs, use required Compose variables, reset development ports, mount
      read-only TOML, and isolate staging state with a separate project.
- [ ] Run unit tests and validate merged Compose using synthetic credentials.
      Assert exact images, internal ports, mounts, restart/logging behavior and
      origin consistency from the resolved JSON without printing credentials.
- [ ] Document manual invocation, prerequisites, bootstrap, DNS/trust and limits.
- [ ] Commit implementation with the required co-author trailer.

### Task 2: Server installation and delivery

**Interfaces:** consumes Task 1 assets and existing server Doppler `photo-proofing/stg`.

- [ ] Verify authorized private-image pulls without exposing credentials. Stop
      deployment if the operator must grant access; do not broaden permissions.
- [ ] Install non-secret runtime config without overwriting existing files.
- [ ] Deploy one staging-tagged release using the launcher under Doppler.
- [ ] Verify health and HTTPS with the public Caddy root certificate; verify
      persistence across recreation, without deleting volumes or reading secrets.
- [ ] Obtain independent review of design compliance and correctness, address
      material findings, and record a focused refactor pass.
- [ ] Report actual running state and any operator prerequisites honestly.

If registry authorization is unavailable, install and verify non-secret assets
only, mark deployment blocked, and leave the manual start procedure ready.
