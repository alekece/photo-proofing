# Standard Staging Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace bespoke deployment with Ansible and migrate staging without
losing the existing account, photos or trusted CA.

**Architecture:** Run Ansible locally on the server under Doppler. Standard
Ansible tasks read non-secret TOML, resolve one release, validate and pull the
Compose stack, then reconcile it with `community.docker` modules.

**Tech Stack:** Ansible Core, community.docker, Docker Compose,
Doppler, Caddy, existing Python 3.14 on the server.

**Spec:** `docs/superpowers/specs/2026-11-03-staging-server-design.md`

**Issue:** https://github.com/alekece/photo-proofing/issues/137

## Global Constraints

- Server config is `/etc/pp/config.toml`; Compose project is `pp-staging`.
- Root `config.example.toml` is the only example; retain development defaults.
- Preserve `https://pp.server.home`, LAN-only binding and existing external volumes.
- Never display/persist secret values; secret-bearing tasks use `no_log`.
- No database reset, credential rotation, public exposure or Hermes automation.

## Review Focus

- Project migration must not create new database/photo/CA volumes.
- TOML changes must recreate API/worker, not leave stale in-memory configuration.
- Password URL encoding must cover `/`, `@`, `%`, `?`, `#`, `:` and spaces.
- Failed validation/pulls must not stop the old stack.
- A second run must not recreate healthy containers unnecessarily.

### Task 1: Standard deployment assets

**Files:** create `deploy/staging.yml`, `deploy/requirements.yml`,
`deploy/requirements.txt`, `deploy/verify.yml`; modify `compose.staging.yml`,
`.github/workflows/ci.yml`, `README.md`, `config.example.toml`; delete the
staging-only TOML example and Python launcher/tests.

**Interfaces:** playbook consumes server-owned TOML, Doppler password, LAN binding,
and explicit existing volume names; installs Compose assets in `/srv/apps/pp-staging`.

- [x] Add a configuration verification playbook covering secret encoding, internal
      ports, pinned images and named external volumes; observe missing-asset failure.
- [x] Declare/install Ansible and collection requirements, then inspect maintained
      module/filter capabilities before implementing.
- [x] Implement local Ansible tasks using maintained Compose pull/up modules,
      non-secret config installation, config checksums for recreation, and explicit
      external volume references. Require existing volume names rather than guess.
- [x] Replace launcher-specific CI tests with Ansible syntax/config validation and
      Caddy validation using synthetic fixtures, never real credentials.
- [x] Update documentation and remove obsolete script/config references.
- [x] Run verification playbook, syntax check, actionlint and diff hygiene; commit.

### Task 2: Safe server migration and review

**Interfaces:** consumes Task 1 assets and existing server volumes.

- [x] Record volume names, account/photo state and public CA fingerprint.
- [x] Install preserved runtime config/public CA under `/etc/pp`; prepare replacement
      assets, validate configuration and pull before stopping old containers.
- [x] Stop old stack without removing volumes; reconcile `pp-staging` against the
      same volumes. If startup fails, release new port binding and restart old stack.
- [x] Verify trusted HTTPS, authenticated login, unchanged CA, data/photo markers,
      internal-only ports and all services healthy; remove synthetic markers.
- [x] Run deployment twice and compare container IDs to verify idempotence; change
      a non-secret TOML setting and prove only API/worker reload as expected.
- [x] Obtain one fresh independent review and address material findings; perform a
      dedicated scoped refactor pass, update existing PR and retain worktree.

## Additional user request

The user ultimately selected no seeded account: fresh databases retain the
first-time setup page. Deployment never creates accounts or manages account
credentials. Preserve existing account/data during migration; no implicit reset.

## Migration evidence

The server now runs `pp-staging` at `https://pp.server.home`, using
`/etc/pp/config.toml` and the non-secret volume/binding settings in
`/etc/pp/staging.yml`. The durable deployment assets are under
`/srv/apps/pp-staging`. All four historical volumes were reused, and the old
containers remain stopped for recovery.

The account/workspace fingerprint, database/photo markers and Caddy root
certificate survived migration unchanged. Synthetic markers were removed.
Authenticated HTTPS login, secure session/status/logout, health and internal-only
ports were verified. A second Ansible run reported `changed=0` with unchanged
container IDs. A temporary TOML log-level change recreated only API/worker; the
original settings were restored.

Syntax/configuration checks, actual invalid-input rejection, special-character
URL encoding, Caddy validation and actionlint passed. The independent reviewer
found a Python dependency gap: the maintained Docker volume module needs
`requests`, not the Docker SDK. The isolated-venv failure was reproduced, then
fixed by declaring `requests` and selecting the same interpreter for local
modules. Configuration verification now exercises that module to prevent
regression. The final deployed repeat run again reported `changed=0`.

The scoped refactor pass retained standard Ansible tasks and one shared
secret-bearing environment block; no additional abstraction or refactor was
warranted. Client trust/firewall acceptance and GHCR privacy review remain owner
steps. No application requirement, password rule or account-seeding behavior
was changed.
