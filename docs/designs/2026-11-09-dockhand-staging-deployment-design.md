# Dockhand staging deployment

Status: Draft, awaiting approval.

Related: [#288](https://github.com/alekece/photo-proofing/issues/288). Replaces the
Ansible deployment described in
[LAN staging deployment](2026-11-03-staging-server-design.md); image publication
in [Pre-production container publication](2026-11-03-preproduction-deployment-design.md)
is unchanged.

## Intent

The private server will host several projects. The owner wants one dashboard that
shows whether each is running, healthy and on which version, instead of asking an
agent or reading Ansible output. [Dockhand](https://dockhand.pro) provides that
dashboard and deploys Compose stacks from Git. Staging deployment moves into it
and the Ansible playbook is retired. This is deployment tooling only; product
requirements do not change.

## Goals, non-goals, anti-goals

Goals:

- A new `staging` build reaches the server without GitHub connecting to it.
- Dockhand shows health, logs and the running image tag; a monitor alerts when
  staging is not on the newest published commit.
- Secrets stay in Doppler and never reach Git, logs or chat.
- Existing volumes, data and the Caddy CA survive the move.

Non-goals: production deployment, moving other projects (Postiz) into Dockhand,
changing images or application behavior.

Anti-goals: a design that needs inbound access to the server, stores real
hostnames, addresses or volume names in Git, or replaces a verified stack with an
empty one.

## Approaches

- **Chosen: moving `:staging` image tag with Dockhand auto-update.** After
  verifying the three `sha-` images, CD also tags them `:staging`. The Dockhand
  stack follows the Compose file on `main` and uses the `:staging` images;
  Dockhand's scheduled auto-update recreates a container when its tag's digest
  changes. No bot commits, no extra branch, no inbound access. Chosen by the
  user as the idiomatic Dockhand pattern. **Gate:** a trial must first show that
  auto-update works correctly on containers belonging to a Compose stack
  (it may recreate them outside the Compose definition). If it does not, use the
  fallback below.
- Fallback: a `deploy/staging` branch written by CD with a
  `PP_IMAGE_TAG=sha-<commit>` env file, which Dockhand syncs. It keeps exact
  pinning and a Git audit trail at the cost of one bot commit per release.
- Not chosen: set the tag through Dockhand's API from a server-side timer
  (undocumented, full-access token).
- Not chosen: GitHub calls a Dockhand webhook (needs inbound access).
- Not chosen: keep Ansible with Dockhand as a dashboard only.

This revises the publication design's rule against moving image aliases for the
staging consumer only. The `:staging` tag is moved only after every `sha-` image
is verified; the three tags are pushed back to back, so skew between services is
seconds on a staging system. The Git tag `staging` and the `sha-` tags remain
the immutable record, and images keep their revision label and embedded build SHA.

## Design

### Image tags

The CD step runs last, after all three images are verified and the `staging` Git
tag advances. It points `:staging` on `pp-api`, `pp-image-worker` and `pp-web`
at the commit's `sha-` image, and never moves it to an older commit (same
ordering rule as the Git tag, serialized by the existing concurrency group).
Rollback means re-tagging `:staging` or pinning a `sha-` tag in Dockhand.

### Dockhand and the Git stack

Dockhand runs on the private server, reachable only on the LAN or VPN, with
authentication enabled. It has Docker socket access, so its UI must never be
exposed publicly. A read-only deploy credential for the repository is stored in
Dockhand's Git settings, never in the repository.

The `pp-staging` stack tracks the Compose file on `main` with scheduled sync and
auto-update enabled for its application containers. It keeps the
Compose project name `pp-staging` and the existing external volumes, so
Compose adopts the running installation rather than creating a new one. Dockhand
must manage the Docker daemon where staging runs today (the root daemon).

### Secrets and server-specific values

- Secrets: Dockhand's Doppler provider is bound to the stack with a read-only
  service token for the staging config. Doppler holds `POSTGRES_PASSWORD` and a
  pre-encoded `PP_DATABASE_URL`, since Compose cannot URL-encode. Dockhand injects
  them at deploy time and masks them in its UI. The token is the only credential
  entered in Dockhand.
- Non-secret, installation-specific values (public origin, LAN binding, volume
  names) are stack environment variables in Dockhand, not in Git. Application
  settings remain in the server-owned `/etc/pp/config.toml`, bind-mounted read-only.

### What the playbook did that changes

| Playbook behavior | After |
|---|---|
| Resolve `staging` tag via `gh` | Dockhand auto-update follows the `:staging` image tag |
| Pull, then reconcile | Dockhand sync, auto-update and Compose |
| Validate TOML, HTTPS origin, RFC1918 binding, four volumes | Dropped from automation; Compose still fails on missing required variables and missing external volumes. Origin and binding checks move to the README checklist. |
| Config and proxy checksum labels reload services on change | Lost; after editing the TOML or Caddy file, redeploy the stack from Dockhand. |
| Export the public Caddy CA | Manual `docker cp` of the public certificate, documented. |

### Failures stay visible

Staging exists to expose broken changes quickly, so a bad build must show as
broken, never be masked by an older version. Dockhand's auto-update is configured
without automatic rollback and without scan-based blocking for these images; a
failing release leaves unhealthy containers on the dashboard, and the
version monitor reports the mismatch. Recovery is a deliberate action: fix
forward, or re-tag `:staging` or pin a `sha-` tag.

### Compose file

The repository Compose files reference `:staging` images, with the values
described below supplied as variables, never committed. Whether Dockhand deploys
the two Compose files or CD/the repo provides one is left to the builder.

### Version visibility

Dockhand shows the running image tag and health per container. The web footer
already shows the build SHA. An Uptime Kuma monitor, delivered separately once the
deployment works, compares the running SHA with the `staging` tag and alerts on
mismatch. Services need healthchecks so Dockhand can report them.

## Migration and rollback

1. Verify the Doppler provider and a trial stack without touching staging.
2. Prepare the deploy branch and Dockhand stack with the real values set in the
   UI; check what it would change before applying.
3. Stop `pp-staging-deploy.timer`, then deploy the stack from Dockhand. The
   project name and volumes are unchanged; a brief outage is acceptable.
4. Verify login, trusted HTTPS with the existing CA, data and photo persistence,
   and a second sync that recreates nothing.
5. Retire `deploy/`, its CI checks and the Ansible documentation only after the
   user accepts the result.

Rollback before step 5: stop the Dockhand stack and re-enable the timer. Database
migrations are not reversible, as before.

## Security

- The Dockhand UI is LAN/VPN only with authentication and, preferably, a Docker
  socket proxy.
- Tokens: Doppler read-only service token per config; read-only repository
  credential. Never in Git, logs or chat. The CD token is write-scoped to the
  deploy branch job only.
- Real hostnames, addresses, volume names and secret-manager identifiers stay out
  of Git, issues and logs; documentation uses reserved examples.

## Left to the builder

- Whether Dockhand deploys two Compose files directly or the CD job produces one
  file; the result must contain no values or secrets.
- Exact env file name and bundle layout.
- Wording of the README section, which replaces the Ansible instructions.
- Scheduled sync interval (a few minutes).

## Verification

- A CD run on `main` publishes images, then moves `:staging` on all three; an older
  run finishing late does not regress it.
- A deliberately broken image leaves staging unhealthy, not rolled back.
- The auto-update trial passes before implementation proceeds.
- Staging redeploys from the registry alone, with no inbound access.
- No secret or installation detail appears in Git or logs.
- CI no longer references the removed playbook.
