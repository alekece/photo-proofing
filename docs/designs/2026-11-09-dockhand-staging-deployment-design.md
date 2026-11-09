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

- **Chosen: GitOps through a deploy branch.** After publishing images, the CD
  workflow commits the image tag to a `deploy/staging` branch. Dockhand syncs the
  branch on a schedule. Git records every deploy; rollback is a revert; the server
  only reads from GitHub. Chosen by the user after comparing it with the others.
- Not chosen: set the tag in Dockhand through its API from a server-side timer.
  The manual does not document it, the API token has full access, and the deploy
  history would live outside Git.
- Not chosen: a moving image tag such as `:staging`. The publication design
  already rejects it; images can briefly mismatch and it does not identify a commit.
- Not chosen: GitHub calls a Dockhand webhook. It needs inbound access to the
  private server. The webhook stays available if a tunnel is ever accepted.
- Not chosen: keep Ansible and use Dockhand only as a dashboard. The user wants
  Dockhand to own deployment.

## Design

### Deploy branch

`deploy/staging` is an orphan branch updated only by the CD workflow. It holds the
deployment bundle for one commit: the Compose definition, proxy configuration and
an env file setting `PP_IMAGE_TAG=sha-<commit>`. Building the bundle from the
tested commit keeps Compose and images consistent.

The bundle contains variable references only, never values: no secrets,
hostnames, addresses or volume names. Dockhand's stack redeploys when files in its
compose directory change, so each release commit triggers one reconciliation and
Compose recreates only containers whose configuration changed.

The new CD step runs last, after all three images are verified and the `staging`
tag is advanced. It uses the workflow's own `GITHUB_TOKEN`, is serialized with the
existing concurrency group and never moves the branch to an older commit,
mirroring the tag rule. Pushes to this branch must not trigger CI or CD. The
existing `staging` tag stays as the published-artifact pointer.

### Dockhand and the Git stack

Dockhand runs on the private server, reachable only on the LAN or VPN, with
authentication enabled. It has Docker socket access, so its UI must never be
exposed publicly. A read-only deploy credential for the repository is stored in
Dockhand's Git settings, never in the repository.

The `pp-staging` stack tracks `deploy/staging` with scheduled sync. It keeps the
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
| Resolve `staging` tag via `gh` | CD writes the tag to the deploy branch |
| Pull, then reconcile | Dockhand sync and Compose |
| Validate TOML, HTTPS origin, RFC1918 binding, four volumes | Dropped from automation; Compose still fails on missing required variables and missing external volumes. Origin and binding checks move to the README checklist. |
| Config and proxy checksum labels reload services on change | Lost; after editing the TOML or Caddy file, redeploy the stack from Dockhand. |
| Export the public Caddy CA | Manual `docker cp` of the public certificate, documented. |

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

- A CD run on `main` publishes images, then advances the deploy branch; an older
  run finishing late does not regress it.
- Pushes to the deploy branch do not start CI or CD.
- Staging redeploys from the branch alone, with no inbound access.
- No secret or installation detail appears in Git or logs.
- CI no longer references the removed playbook.
