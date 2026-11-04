# PhotoProofing

A self-hosted workspace for a photographer to share private photo proofs, collect client favorites, export a selection for external retouching, and deliver finished photos through the same gallery link.

The planned MVP is a local-network workflow trial with replaceable JPEG photos, one photographer account/workspace, and responsive photographer/client screens. Billing is outside the product's scope.

## Project status and documentation

Development is restarting around the [product specifications](docs/SPECIFICATIONS.md) and [MVP scope and acceptance checklist](docs/releases/MVP.md). These documents describe the target product, not completed features.

The repository supports one-time photographer account setup, email/password login, a protected dashboard, persistent revocable sessions, and persistent galleries with autosaved settings (title, layout, expiry, included photo count, watermark flag) backed by SQLx/PostgreSQL. Auth screens use Tailwind and selected shadcn-svelte components. Configuration/logging, API health, a durable PostgreSQL job queue run by the worker, in-memory notification domain code, private client links (the photographer shares a gallery from its page; `/g/<token>` shows its title, expiry and its ready proofs to anyone with the link; reload the page to see newly processed photos), and a proof viewer also exist. The photo API accepts JPEG uploads and preserves originals. The worker then renders a 400 px thumbnail and a 1600 px preview: upright, sRGB, and stripped of metadata. Failed photos can be retried, and the API serves the derivatives of ready photos to the signed-in photographer and to holders of a valid client link, never the original. Signed-in pages also receive live photo changes over Server-Sent Events (`GET /api/v1/events`), announced by PostgreSQL triggers. A gallery's Photos tab uploads dropped or picked JPEGs through an app-wide queue that keeps sending while the photographer moves between dashboard pages, shows each photo's upload and processing state live, and deletes any photo whatever its state. The hero picker and client delivery are not implemented.

- [Product specifications](docs/SPECIFICATIONS.md): requirements, constraints, and exclusions.
- [MVP release](docs/releases/MVP.md): first-release scope and acceptance evidence to collect.
- [ADR 0001 — Technology baseline](docs/adr/0001-technology-baseline.md): established choices and tradeoffs.
- [ADR 0003 — Private photo storage](docs/adr/0003-photo-storage.md): photo directory layout and atomic writes.
- [Agent guidance](AGENTS.md): shared development and documentation rules, with [Rust](crates/AGENTS.md) and [frontend](web/AGENTS.md) guidance.
- [Approved design records](docs/designs/): designs retained with the repository.

## Prerequisites

For local development:

- A current stable Rust toolchain supporting edition 2024.
- The libvips development library and pkg-config for API and worker builds: `sudo apt-get install libvips-dev pkg-config`.
- Nightly rustfmt for the repository's import-grouping configuration: `rustup toolchain install nightly --component rustfmt`.
- Node.js 24.15 or newer in the Node 24 line (required by the frontend lockfile) and pnpm 10.13.1, matching the frontend CI setup. With Corepack available: `corepack enable` and `corepack prepare pnpm@10.13.1 --activate`.
- PostgreSQL 17 for application account/session storage and database tests. Compose supplies a development service.

For the container stack, use Docker with Compose and BuildKit support. The worker currently uses Unix signal handling; use a Unix environment or the Linux containers for it.

## Run the current scaffold

All commands below run from the repository root. They start the existing scaffold; they do not establish MVP acceptance.

### Docker Compose

If `.env` does not already exist, copy the local-only sample:

```sh
cp .env.example .env
```

Review the sample credentials and host ports before starting. Keep `.env` out of Git; do not use these sample values as production credentials.

```sh
docker compose config --quiet
docker compose up --build --wait
docker compose ps
```

Open `http://localhost:8000/login` and create the installation's single account. Subsequent visits offer sign-in. Use that exact origin; `PP_APP_ORIGIN` must match the scheme, hostname, and port used by the browser. The proxy forwards `/api` to the API and other paths to the frontend. Check the API health endpoint:

```sh
curl http://localhost:8000/api/v1/health
```

Expected response: `{"status":"ok"}`. The Compose web container serves the Vite development server for local use; the separate production image runs the SvelteKit Node adapter. Health checks establish process/readiness checks, not the complete proofing workflow.

The local Compose host ports bind to `127.0.0.1`. Other devices on the local
network cannot reach them using these defaults. See [LAN staging deployment](#lan-staging-deployment)
for the separate HTTPS deployment; the development defaults do not expose the
service publicly.

Stop the local stack with `docker compose down`. This preserves the named
PostgreSQL and photo volumes. `docker compose down -v` deletes both volumes and
their contents. The `photo-data` volume is mounted at `/var/lib/pp/photos` in the
API and worker. Both refuse to start if `PP_PHOTO_DIR` is not writable; natively
it defaults to `./data/photos`, which must exist and be shared by both processes
(see [ADR 0003](docs/adr/0003-photo-storage.md)).

## Published images

Successful `main` builds publish private images as `ghcr.io/alekece/pp-api`,
`ghcr.io/alekece/pp-image-worker`, and `ghcr.io/alekece/pp-web`, each tagged
`sha-<full-commit-sha>`. The Git tag `staging` points to the newest commit with
all three images published and advances only forward. The production web image
listens on port `3000`; set `PP_API_INTERNAL_URL` to the API's internal URL and
route `/api/*` requests to the API.

### LAN staging deployment

[deploy/staging.yml](deploy/staging.yml) uses Ansible's maintained
`community.docker` Compose modules to pull and reconcile one published commit.
Run it locally on the server from a checkout containing these deployment assets.
Prerequisites: Python 3.12+, Docker Compose 2.24.4+, working Docker access, GitHub
CLI access to the repository, an operator-selected Doppler project/config, and sudo
for `/etc/pp`.
Private GHCR images also require authorized Docker registry credentials; never
paste tokens into commands or logs or change visibility to bypass authentication.

Install the declared dependencies in a dedicated environment (install your OS's
`python3-venv` package first if needed):

```sh
python3 -m venv ~/.local/share/pp-ansible
. ~/.local/share/pp-ansible/bin/activate
python3 -m pip install -r deploy/requirements.txt
ansible-galaxy collection install -r deploy/requirements.yml
```

Use the single root [config.example.toml](config.example.toml) as the reference
for server-owned `/etc/pp/config.toml`. Preserve its existing values on upgrades.
For containers set `host = "0.0.0.0"`, `port = 8080`, and
`photo.dir = "/var/lib/pp/photos"`. Omit `[database]`: Doppler supplies the password
and Ansible derives the URL. Set `session.secure_cookie = true` and
`auth.app_origin = "https://proofing.example"` (replace this reserved example with
your HTTPS hostname without a port, path or trailing slash). Proxy and frontend origins derive from this
setting, and checksum labels reload API/worker when the TOML changes.

Create a **private**, server-owned `/etc/pp/staging.yml` with the binding
and four existing volume names. The following values are examples, not an inventory:

```yaml
pp_bind_address: 127.0.0.1
pp_volumes:
  postgres: example-postgres
  photos: example-photos
  caddy_data: example-caddy-data
  caddy_config: example-caddy-config
```

The project is named **`pp-staging`**. Supply the actual existing volumes for your
installation; they are external volumes, not recreated or renamed. A fresh
installation must explicitly provision its four empty Docker volumes and supply their names;
the playbook refuses missing volumes to prevent accidental empty replacements.
For fresh setup, initially use `pp_bind_address: 127.0.0.1` and a trusted SSH tunnel
that preserves the configured origin. No account is seeded: the first-time setup
page creates the owner account. Do that before enabling LAN access.

Deploy manually, with the virtual environment active:

```sh
doppler run --project "$DOPPLER_PROJECT" --config "$DOPPLER_CONFIG" \
  --only-secrets POSTGRES_PASSWORD --no-fallback -- \
  ansible-playbook -i localhost, deploy/staging.yml -e @/etc/pp/staging.yml
```

Set `DOPPLER_PROJECT` and `DOPPLER_CONFIG` privately on the server before running.
Add `--ask-become-pass` if sudo requires a password. If the system's locale is
unavailable, set `LC_ALL=C.UTF-8` for Ansible. Tasks handling credentials or
installation metadata use `no_log`; do not enable fact caching or verbose/debug
logging with real settings. Doppler protects stored secrets, not their runtime
environment from root/Docker admins.
Ansible does not create accounts or manage their credentials in Doppler.

The deployment bundle is installed under `/srv/apps/pp-staging`. Public Caddy CA
export: `/etc/pp/staging-root.crt`; install it into each client's trust store.
Never export its private key. Configure private DNS so your chosen hostname resolves
to the reserved server IP. Only TCP 443 is published, bound to the chosen IPv4
address. A LAN binding is not a firewall; verify network restrictions and do not forward router
ports. DNS may also return IPv6, but this deployment only publishes IPv4.

The playbook validates and pulls before reconciliation and never builds images.
Failed pulls leave running containers unchanged; failed startup can leave a
partially updated stack. PostgreSQL migrations are not automatically reversible.
Do not delete volumes or rotate the password without updating PostgreSQL.
Health checks are not complete LAN/browser acceptance; verify login and trusted
HTTPS with the public CA, not `--insecure`. Hermes automation is not configured.

For the one-time project/path migration, keep the old deployment bundle, config
and volume-name record until verification is complete. First run the new playbook
with `-e pp_config_source=/path/to/existing/config.toml -e pp_prepare_only=true`
to install preserved settings, validate and pull without starting the replacement.
Stop the old project's containers without deleting volumes, then run normally.
If startup fails, stop `pp-staging` containers to release the port and restart the
old containers; do not run two PostgreSQL instances against the same volume.
The old public root remains valid because the Caddy data volume is shared.

Deployment tooling remains in this repository, but real hostnames, addresses,
volume mappings, account details, secret-manager identifiers and certificates
must remain outside Git. This also applies to CI fixtures, documentation, PR/issue
text and shared logs. Use reserved `.example` hostnames and synthetic values in
public examples. Keep runtime configuration and deployment variable files under
`/etc/pp`; they contain private installation details even when they contain no
passwords. The local `deploy/*.local.yml` pattern is ignored as an additional
safeguard, not a substitute for reviewing changes before committing.

Validate deployment assets without production secrets:

```sh
ansible-playbook -i localhost, deploy/staging.yml --syntax-check
ansible-playbook -i localhost, deploy/verify.yml
```

### Native development

Start a local PostgreSQL 17 instance first, or start only the Compose database with `docker compose up -d postgres`. Native defaults use the sample account/database from `.env.example` at port 5432. Set `PP_DATABASE_URL` when using another database or port. Create the photo directory once with `mkdir -p data/photos` (gitignored) or point `PP_PHOTO_DIR` elsewhere; the API and worker must use the same directory. The API connects and applies embedded migrations before listening; it fails startup if the database is unavailable. The worker also connects and applies the migrations at startup, then polls the `jobs` table; on SIGTERM it stops claiming and gives running jobs a grace period.

Install frontend dependencies:

```sh
pnpm --dir web install --frozen-lockfile
```

Run each process in its own terminal; stop with `Ctrl-C`:

```sh
cargo run -p pp-api
cargo run -p pp-image-worker
pnpm --dir web dev
```

Defaults: API at `http://127.0.0.1:8080`, frontend at `http://localhost:5173`. The frontend proxies `/api` to the API. The dashboard, which lists and creates galleries, is at `/dashboard`; a gallery's page creates its private client link, which opens `/g/<token>`; the client page shows the gallery's ready proofs; the gallery page's Photos tab uploads and deletes photos.

```sh
curl http://127.0.0.1:8080/api/v1/health
```

Open `http://localhost:5173/login` for native setup/sign-in. Account setup closes after the first account, including concurrent attempts, but until then anyone who can reach the API can create it: run setup before exposing the host to other devices. Sign out revokes that particular session, including any copy of its cookie; independent sessions remain signed in. The current HTTP trial sends passwords and cookies unencrypted: use a trusted network and synthetic credentials that you do not reuse. Do not use real client photos.

## Configuration

Rust process settings use this precedence: CLI flags, environment variables, optional TOML file, defaults. Copy [config.example.toml](config.example.toml) to the ignored `config.toml` if needed. For example:

```sh
cargo run -p pp-api -- --config config.toml
```

API settings include `PP_HOST`, `PP_PORT`, `PP_LOG_LEVEL`, `PP_DATABASE_URL`, `PP_APP_ORIGIN`, `PP_SESSION_IDLE_TIMEOUT`, `PP_SESSION_COOKIE_SECURE` (default `false`; set to `true` behind HTTPS), `PP_SESSION_COOKIE_NAME` (default `pp_session`), upload bounds `PP_PHOTO_UPLOAD_MAX_SIZE` (64 MiB), `PP_PHOTO_UPLOAD_MAX_PIXELS` (100,000,000) and `PP_PHOTO_UPLOAD_CONCURRENCY` (4), live event streams `PP_EVENTS_KEEP_ALIVE` (default `15s`, also how soon a signed-out stream ends), `PP_EVENTS_MAX_STREAMS` (256) and `PP_EVENTS_MAX_STREAMS_PER_WORKSPACE` (32), and the sign-in limiter settings `PP_LIMITER_MAX_FAILURES` (default 10), `PP_LIMITER_WINDOW` (default `1m`) and `PP_LIMITER_MAX_PEERS` (default 1024); worker queue settings are `PP_QUEUE_POLL_INTERVAL` (default `5s`), `PP_QUEUE_LEASE` (`60s`), `PP_QUEUE_CONCURRENCY` (2), `PP_QUEUE_BACKOFF_BASE` (`10s`), `PP_QUEUE_BACKOFF_CAP` (`5m`) and `PP_QUEUE_SHUTDOWN_GRACE` (`30s`), the worker rejects originals above `PP_PROCESS_MAX_PIXELS` (100,000,000; keep it equal to the upload limit), and the worker needs `PP_DATABASE_URL` too; both Rust processes accept `PP_CONFIG` and logging configuration. Use `--help` for process-specific flags. Frontend development proxy target: `PP_API_PROXY_TARGET` (default `http://127.0.0.1:8080`). Server-side auth checks use `PP_API_INTERNAL_URL` with the same native default; Compose sets it to `http://api:8080`. These frontend settings use its Node runtime, not the Rust TOML file.

Upload admission is shared across API workers and reserved during request extraction. When all slots are occupied, an upload returns `429` with `Retry-After` without waiting, potentially before gallery or header validation; when the disk is full it returns `507`. `GET /api/v1/photo-upload/limits` tells the signed-in browser the concurrency, size and pixel limits, `DELETE /api/v1/galleries/{id}/photos/{photo_id}` deletes a photo, its jobs and its files at any stage, and `POST /api/v1/galleries/{id}/photos/delete` with `{"photo_ids": [...]}` does the same for up to 1000 photos of the gallery, answering how many it deleted. Authorization still precedes body consumption. A slot remains reserved through persistence and cleanup, and is released on completion, rejection, or cancellation.

Uploads also have a fixed 30-second idle timeout between body chunks; an idle upload receives HTTP 408 and its staged file is discarded.

Session configuration uses human-readable durations through the existing configuration parser:

```toml
[session]
idle_timeout = "30d"
secure_cookie = false

[auth]
app_origin = "http://localhost:5173"
```

Equivalent overrides: `PP_SESSION_IDLE_TIMEOUT="12h"` or `--session-idle-timeout "1h 30m"`. Set `PP_SESSION_COOKIE_SECURE=true` or `--session-cookie-secure true` when serving the app over HTTPS; the default is false for local HTTP. The timeout must be at least one second and fit cookie/timestamp arithmetic. Sessions persist across browser restarts. User navigation/interactions renew them with coalescing; passive status requests/background traffic do not. There is no absolute age limit or Remember me checkbox. A timeout change applies to new logins and subsequent renewals. See [ADR 0002](docs/adr/0002-account-sessions.md).

Auth mutations require the configured exact Origin; other/missing origins are rejected, including on login/setup. Browser API calls send their HttpOnly cookie automatically. Credential bodies, hashing concurrency, and sign-in attempts are bounded. The in-process limit defaults to 10 failed credential attempts per peer IP per minute (any rejected login or setup request counts, including malformed or oversized bodies, except `429` refusals and server errors; configurable via the `PP_LIMITER_*` settings; successful sign-ins do not count); behind the current proxy this may be shared by all clients, so one device can still block sign-in with repeated failures, and it resets on API restart. Arbitrary forwarded IP headers are not trusted. At most two password hashes run at once and further sign-in or setup requests are refused with 429 without counting as failures, and the limiter tracks at most `PP_LIMITER_MAX_PEERS` peers (new peers are refused while it is full); neither is a substitute for request throttling, which a public deployment should add at an API gateway.

Compose reads its development settings from [.env.example](.env.example)/`.env`: `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB`, `PP_LOG_LEVEL`, `PP_APP_ORIGIN`, `PP_SESSION_IDLE_TIMEOUT`, `PP_SESSION_COOKIE_SECURE`, and the `PP_*_PORT` variables. Changing a published proxy port requires updating `PP_APP_ORIGIN` to match. If database credentials contain URL-special characters, supply a correctly encoded connection URL rather than treating it as plain text. The native Rust TOML configuration is separate; copying it does not configure Compose.

## Verify and build

Choose checks relevant to the change; report checks that could not run. CI definitions live in [.github/workflows/ci.yml](.github/workflows/ci.yml).

```sh
cargo +nightly fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
```

Apply formatting with `cargo +nightly fmt --all`. Stable rustfmt ignores the nightly-only import-grouping setting.

```sh
pnpm --dir web format:check   # pnpm --dir web format to fix
pnpm --dir web lint
pnpm --dir web check
pnpm --dir web test
pnpm --dir web build
pnpm --dir web audit --prod
pnpm --dir web verify   # all of the above except audit
```

Database-backed tests use disposable SQLx databases through a test-role `DATABASE_URL` (the role must have permission to create databases); CI already provisions PostgreSQL 17 for Rust tests. For example, after creating a dedicated test database:

```sh
export DATABASE_URL=postgres://USER:PASSWORD@localhost:5432/pp_test
```

`DATABASE_URL` is for SQLx tests; application settings use `PP_DATABASE_URL`. Keep test fixtures synthetic and keep original trial photos elsewhere. Backup/restore tooling is excluded from MVP, but persistence across normal restarts is required.

The focused real-browser auth test expects an isolated running frontend/API and disposable database. With services using `http://localhost:15163` and API port 18063:

```sh
# API terminal: set PP_DATABASE_URL to your disposable database URL too.
PP_PORT=18063 PP_APP_ORIGIN=http://localhost:15163 cargo run -p pp-api
# Frontend terminal:
PP_API_INTERNAL_URL=http://127.0.0.1:18063 PP_API_PROXY_TARGET=http://127.0.0.1:18063 pnpm --dir web dev --host 127.0.0.1 --port 15163
# Test terminal (defaults to /usr/bin/google-chrome; override for your installed browser):
PP_E2E_URL=http://localhost:15163 pnpm --dir web test:auth
```

Use a fresh disposable database to exercise initial setup; reruns verify its persisted `trial@example.test` account with synthetic test credentials. The harness does not reset databases. For the short-idle scenario, start the API with `PP_SESSION_IDLE_TIMEOUT=10s` and run with `PP_E2E_IDLE_MS=10000`; otherwise that scenario is explicitly skipped. `PP_BROWSER_PATH` selects a Chrome/Chromium binary. Browser artifacts go to `/tmp/pp63-playwright-results` by default, override with `PP_E2E_OUTPUT`. Unit/component tests do not substitute for this browser/API/database check. Compose proxy and persistence checks are additional integration evidence, not automatic release acceptance.

For a Rust build-output reset, use `cargo clean`. Frontend generated state lives under `web/node_modules`, `web/.svelte-kit`, and `web/build`; reinstall dependencies after removing dependency state. Do not confuse resetting build outputs with deleting persistent database or photo data.

## Contributing

Use [Conventional Commits](https://www.conventionalcommits.org/). Treat changelog maintenance as release work; no automatic changelog-generation workflow is currently configured.

Follow the [shared Superpowers workflow](AGENTS.md#superpowers-delivery-workflow): brainstorm a bounded outcome, approve the written design, plan, implement in an isolated worktree, verify, request fresh independent review, and open a PR. Brainstorming may start without an issue; link an outcome-focused issue before implementation planning. Name branches `<type>/<issue>-slug` with a git-cliff commit type, such as `feat/61-development-workflow`. Merge only after explicit approval, preferably with a squash merge.

Keep requirements, release acceptance, significant decisions, and implementation details in their respective documents, following [AGENTS.md](AGENTS.md#documentation-and-development-workflow). Preserve committed approved designs. Commit plans on the working branch and retain them through PR review and corrections, then remove them before final merge approval after retaining lasting decisions in permanent records. Product requirements and release scope changes require approval.
