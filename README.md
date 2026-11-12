# PhotoProofing

A self-hosted workspace for a photographer to share private photo proofs, collect client favorites, export a selection for external retouching, and deliver finished photos through the same gallery link.

The planned MVP is a local-network workflow trial with replaceable JPEG photos, one photographer account/workspace, and responsive photographer/client screens. Billing is outside the product's scope.

## Project status and documentation

Development is restarting around the [product specifications](docs/SPECIFICATIONS.md) and [MVP scope and acceptance checklist](docs/releases/MVP.md). These documents describe the target product, not completed features.

The repository supports one-time photographer account setup, email/password login, persistent revocable sessions, and a protected dashboard backed by SQLx/PostgreSQL. Galleries autosave title, layout, expiry, included photo count and watermark flag without repeated success notifications or reloading loaded panels. The dashboard filters draft, shared, delivered, archived and expired states (`GET /api/v1/galleries?state=…`, repeatable and kept in the URL), shows ready-photo and favorite counts, and totals them in the footer. Client selection, download and open activity is recorded as one unread notification per gallery (bell panel, card badge, live over SSE; `GET /api/v1/notifications`), and delivered cards show how many finals were downloaded. Delivering is the only way a gallery becomes delivered; nothing sets archived yet. Auth screens use Tailwind and selected shadcn-svelte components. Every screen, client pages included, is available in English and French: the workspace language is chosen from the top bar (`PUT /api/v1/settings/language`) and read by every page from the public `GET /api/v1/language`; before setup the browser language decides. API errors carry a stable `code` (`{"code": "zip_not_ready"}`, plus English `context` in debug builds only) that the web translates, failed ZIP exports included. Configuration/logging, API health, a durable PostgreSQL worker queue also exist.

The Photos tab uploads dropped or picked JPEGs through an app-wide queue that continues across dashboard pages, shows each tile's upload/processing state, retries failures and confirms deletions. Originals are preserved unchanged. The worker produces upright, sRGB, metadata-free thumbnails and previews at the workspace's proof quality (Settings: low 640/1600 px, medium 800/2000 px by default, high 1000/2400 px; changing it regenerates every proof). Private finals drafts use the same upload/review/removal journey with `?finals=true`; clients cannot see them. Favorites and Final remain disabled until sharing. The Deliver dialog counts finals only, and confirming it (`POST /api/v1/galleries/<id>/deliver`) switches the private link to the ready finals, which clients see but cannot favorite. Finals removed afterwards disappear at once. On a delivered link the client downloads one finished photo (`GET /api/v1/galleries/shared/<token>/photos/<photo>/download`) or all of them as one ZIP: `POST …/shared/<token>/download` prepares it (`200` ready, `202` building, `503` failed) and `GET` streams it. The ZIP is replaced when the finals change and removed with the gallery's expiry or archiving. The first download of each photo and of the ZIP is recorded (`downloaded_at`) and shown live to the photographer on the Final tab, with its date and time over the photo. Favorites record when the client picked each photo (`favorited_at`, shown the same way on the Favorites tab); picking an already picked photo keeps its date.

Sharing creates a private `/g/<token>` link showing the gallery's title, expiry and ready proofs. Clients heart photos through `GET/PUT/DELETE /api/v1/galleries/shared/<token>/favorites[/<photo>]`, with a live count against the included photos; selection locks after delivery. Clients have no event stream and reload to see newly processed proofs. Signed-in pages receive live photo and favorite changes over Server-Sent Events (`GET /api/v1/events`) announced by PostgreSQL triggers. The client page records each open (`POST /api/v1/galleries/shared/<token>/open`) in `last_opened_at`, at most once per minute; the gallery hero shows `Last opened <date>` or `Not opened yet`, and each recorded open adds to an unread "open" notification. The owner uses read-only Preview rather than an Open shortcut that carries the client credential.

Proof photos have shared plain-text conversations in the client and photographer viewers, closed by default, with count badges on tiles. Under `/api/v1/galleries/shared/<token>` (client) or `/api/v1/galleries/<id>` (photographer), `GET /comments` lists counts, `GET/POST /photos/<photo>/comments` reads/adds messages, and `PATCH/DELETE /comments/<comment>` edits/deletes the author's messages. Anyone with the link shares the client identity. Deleted text is hidden from clients but retained, struck through, for the photographer. New client posts add grouped `comment` bell activity and refresh the photographer's open thread via the existing event stream. Clients have no live stream or reply notification; comments stop for them on delivery, expiry or archive. Finals and read-only Preview have no comments.

A photo with prepared clean images can be starred as the gallery hero, used on the dashboard and client cover. Settings (`GET/PUT /api/v1/settings`, `GET /api/v1/settings/logo`) save the studio name, normalized PNG logo, watermark (name/logo/both, repeat/one of nine positions, ten sizes) and cover signature (name/logo/both/none, ten sizes). The client cover and Preview draw the saved signature over the clean 2000 px preview, with the logo above the name for Both; no proof original is exposed. Shared logo access requires an available private link and does not expose private settings.

Every proof upload prepares clean and watermarked thumbnail/preview pairs. Gallery switches choose existing files instantly, without jobs or photo resets. Only client grids/viewers and the client preview (owner routes with `?watermark=true`) use marked files; ordinary photographer surfaces and Settings samples stay clean. Effective watermark changes regenerate only marked copies in all galleries, including disabled ones. Stale marked proofs stay hidden from enabled clients, while clean covers, disabled clients and owner images remain usable with processing/failure/retry feedback. Finals, originals and exports stay unchanged. Proof responses use no-store; published render revisions refresh mounted images.

Favorites offers **Download**, disabled **Downloading…** during preparation, with no separate download-started message. Tile and viewer actions stream an authenticated owner's unchanged proof original (`GET /api/v1/galleries/<id>/photos/<photo>/download`); client links never serve originals. Whole and select-mode subset selections use `POST /api/v1/galleries/<id>/export`, even for one selected photo: an omitted body snapshots all favorite proofs, while `{ "photo_ids": [...] }` snapshots an explicit nonempty favorite subset. Invalid, duplicate or foreign IDs are refused. One ZIP preparation slot per gallery rejects competing requests with 409. The worker archives exactly the persisted snapshot; ZIPs stay available for 15 min, whether or not they were downloaded; asking again for an unchanged selection reuses the ready ZIP (`200`) instead of building a new one (`202`). ZIPs are capped at 2 GiB (see `[export]`).

- [Product specifications](docs/SPECIFICATIONS.md): requirements, constraints, and exclusions.
- [MVP release](docs/releases/MVP.md): first-release scope and acceptance evidence to collect.
- [ADR 0001 — Technology baseline](docs/adr/0001-technology-baseline.md): established choices and tradeoffs.
- [ADR 0003 — Private photo storage](docs/adr/0003-photo-storage.md): photo directory layout and atomic writes.
- [ADR 0004 — Styleable component libraries](docs/adr/0004-styleable-component-libraries.md): prefer well-known libraries such as shadcn-svelte over hand-built components.
- [Agent guidance](AGENTS.md): shared development and documentation rules, with [Rust](crates/AGENTS.md) and [frontend](web/AGENTS.md) guidance.
- [Approved design records](docs/designs/): designs retained with the repository.

## Prerequisites

The photographer's Settings footer identifies the loaded frontend build and can
copy its full commit SHA. Deployment embeds the web image's revision at build
time; local builds say Development build unless `PP_BUILD_SHA` is set to a full
lowercase 40-character commit SHA when running `pnpm --dir web build`. Changing
runtime environment variables does not change an already-built frontend. The
API image carries the same commit and reports it as `commit` from
`/api/v1/health` (`null` for local builds).

For local development:

- A current stable Rust toolchain supporting edition 2024.
- The libvips development library, version 8.15 or newer, and pkg-config for API and worker builds. The worker also needs a TrueType font (such as `fonts-dejavu-core` on Debian or Ubuntu) to draw a text watermark. Distributions that ship it: Debian 13 (trixie), Ubuntu 24.04 and newer, Fedora, Arch and Homebrew. On Debian or Ubuntu: `sudo apt-get install libvips-dev pkg-config`; with Homebrew: `brew install vips pkg-config`. Older releases, such as Ubuntu 22.04 (libvips 8.12), are not supported.
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
listens on port `3000`; `PP_API_INTERNAL_URL` is required (the API's internal URL) and
route `/api/*` requests to the API.

### Staging deployment

Staging is deployed and monitored from a deployment dashboard on the private server
([design](docs/designs/2026-11-09-staging-dashboard-deployment-design.md)). After CI
succeeds on `main`, CD publishes the images and points the `:staging` tags at that commit.
The dashboard tracks [deploy/compose.yml](deploy/compose.yml) on `main` as the Git stack
`pp-staging` and auto-updates the application containers when a `:staging` tag moves.
Failed updates stay failed and unhealthy; nothing rolls back automatically.

Dashboard setup (private LAN/VPN only, authentication enabled; it has Docker socket access):

- Manage the Docker daemon where staging runs and keep the project name `pp-staging`.
- Bind the dashboard's secret-manager integration to the stack with a read-only service token for the
  staging config. The secret manager supplies `POSTGRES_PASSWORD` and a URL-encoded `PP_DATABASE_URL`.
- Set `PP_APP_ORIGIN`, `PP_STAGING_HOST`, `PP_BIND_ADDRESS` and the four existing volume
  names (`PP_POSTGRES_VOLUME`, `PP_PHOTO_VOLUME`, `PP_CADDY_DATA_VOLUME`,
  `PP_CADDY_CONFIG_VOLUME`) as stack variables in the dashboard, never in Git. A fresh
  installation must first create the four empty Docker volumes.
- The dashboard's Git stack must sync the compose file only; enable auto-update on the API,
  worker and web containers, and leave vulnerability-scan blocking off for them.
- Private GHCR images need authorized Docker registry credentials; never paste tokens into
  commands or logs or change visibility to bypass authentication.

Use the single root [config.example.toml](config.example.toml) as the reference
for server-owned `/etc/pp/config.toml`, which the stack mounts read-only. Preserve its
existing values on upgrades. Redeploy the stack after editing it. Check before deploying:

- For containers set `host = "0.0.0.0"`, `port = 8080` and `photo.dir = "/var/lib/pp/photos"`.
- Omit `[database]`; `PP_DATABASE_URL` comes from the secret manager.
- Set `session.secure_cookie = true` and `auth.app_origin = "https://proofing.example"`
  (replace this reserved example with your HTTPS hostname, without a port, path or trailing
  slash). `PP_APP_ORIGIN` must equal it.
- `PP_BIND_ADDRESS` is the private IPv4 address the proxy publishes on TCP 443. A LAN
  binding is not a firewall; verify network restrictions and do not forward router ports.
  For a fresh setup, bind `127.0.0.1` and use a trusted SSH tunnel that preserves the
  configured origin, and create the owner account on the first-time setup page before
  enabling LAN access. No account is seeded.

Caddy issues the internal HTTPS certificate and keeps its CA in the Caddy data volume. Export
the public root with `docker cp pp-staging-proxy-1:/data/caddy/pki/authorities/local/root.crt .`
and install it into each client's trust store. Never export its private key. Configure
private DNS so your hostname resolves to the server. Only IPv4 is published.

Images are never built on the server. A failed pull leaves running containers unchanged.
PostgreSQL migrations are not automatically reversible. Do not delete volumes or rotate the
database password without updating PostgreSQL. Health checks are not complete LAN/browser
acceptance; verify login and trusted HTTPS with the public CA, not `--insecure`.

For the one-time proof-variant upgrade (`0017_proof_variants.sql`), stop all old API and
worker processes before starting new code, which applies migrations at startup. Preserve
database/photo volumes and originals. The migration durably queues ready/processing proofs
for four-file preparation; old files may contain watermarks and are not considered clean.
Unprepared proofs and covers remain unavailable until publication. Failed proofs keep manual
retry, and finals are untouched. Do not restart old two-file workers after this migration;
rolling old/new workers together is unsupported.

Deployment tooling remains in this repository, but real hostnames, addresses,
volume mappings, account details, secret-manager identifiers and certificates
must remain outside Git. This also applies to CI fixtures, documentation, PR/issue
text and shared logs. Use reserved `.example` hostnames and synthetic values in
public examples. Keep runtime configuration under `/etc/pp`; it contains private
installation details even when it contains no passwords. The local `deploy/*.local.yml`
pattern is ignored as an additional safeguard, not a substitute for reviewing changes
before committing.

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

API settings include `PP_HOST`, `PP_PORT`, `PP_LOG_LEVEL`, `PP_DATABASE_URL`, `PP_APP_ORIGIN`, `PP_SESSION_IDLE_TIMEOUT`, `PP_SESSION_COOKIE_SECURE` (default `false`; set to `true` behind HTTPS), `PP_SESSION_COOKIE_NAME` (default `pp_session`), upload bounds `PP_PHOTO_UPLOAD_MAX_SIZE` (64 MiB), `PP_PHOTO_UPLOAD_MAX_PIXELS` (100,000,000), `PP_PHOTO_UPLOAD_CONCURRENCY` (4) and `PP_PHOTO_UPLOAD_RETRY_AFTER` (default `1s`, the `Retry-After` of a busy answer), `PP_PHOTO_UPLOAD_BODY_IDLE_TIMEOUT` (default `30s`, 1s to 10m), selection ZIP exports `PP_EXPORT_MAX_SIZE` (default 2 GiB, the most original bytes a selection may hold; the API and worker must agree) and `PP_EXPORT_RETENTION` (default `15m`, how long a ZIP stays downloadable), live event streams `PP_EVENTS_KEEP_ALIVE` (default `15s`, also how soon a signed-out stream ends), `PP_EVENTS_CAPACITY` (events buffered per stream before it is told to resync, default 1024), `PP_EVENTS_RELAY_BACKOFF_START` (`250ms`) and `PP_EVENTS_RELAY_BACKOFF_CAP` (`30s`) for reconnecting the single database listener, `PP_AUTH_HASH_CONCURRENCY` (default 2), `PP_QUEUE_MAX_ATTEMPTS` (default 5; the API records it on each job it enqueues, so the worker does not read it), and the sign-in limiter settings `PP_LIMITER_MAX_FAILURES` (default 10), `PP_LIMITER_WINDOW` (default `1m`) and `PP_LIMITER_MAX_PEERS` (default 1024); worker queue settings are `PP_QUEUE_POLL_INTERVAL` (default `5s`), `PP_QUEUE_LEASE` (`60s`), `PP_QUEUE_CONCURRENCY` (2), `PP_QUEUE_BACKOFF_BASE` (`10s`), `PP_QUEUE_BACKOFF_CAP` (`5m`) and `PP_QUEUE_SHUTDOWN_GRACE` (`30s`), the worker rejects originals above `PP_PROCESS_MAX_PIXELS` (100,000,000; keep it equal to the upload limit), and the worker needs `PP_DATABASE_URL` too; both Rust processes accept `PP_CONFIG` and logging configuration. Use `--help` for process-specific flags. Frontend development proxy target: `PP_API_PROXY_TARGET` (default `http://127.0.0.1:8080`). Server-side requests to the API use `PP_API_INTERNAL_URL`: `pnpm dev` defaults it to `http://127.0.0.1:8080`, while a production server without it refuses to start, with an error naming the variable. Compose sets it to `http://api:8080`. These frontend settings use its Node runtime, not the Rust TOML file.

Comment settings are `PP_COMMENTS_MIN_INTERVAL` (default `2s`, range `1s`–`1m`) and `PP_COMMENTS_MAX_PER_PHOTO` (default 100, range 1–65535), also available under `[comments]` in TOML. Client posts, edits and deletions share the in-memory per-gallery interval; faster requests receive `429 busy` with `Retry-After`. Client posts at the thread cap receive `409 thread_full`; the cap counts deleted messages and photographer replies. The photographer is exempt from both limits. Comments are trimmed to 1–1000 Unicode characters.

Upload admission is shared across API workers and reserved during request extraction. When all slots are occupied, an upload returns `429` with `Retry-After` without waiting, potentially before gallery or header validation; when the disk is full it returns `507`. `GET /api/v1/upload/limits` tells the signed-in browser the concurrency, size and pixel limits, `DELETE /api/v1/galleries/{id}/photos/{photo_id}` deletes a photo, its jobs and its files at any stage, and `POST /api/v1/galleries/{id}/photos/delete` with `{"photo_ids": [...]}` does the same for up to 1000 photos of the gallery, answering how many it deleted. Authorization still precedes body consumption. A slot remains reserved through persistence and cleanup, and is released on completion, rejection, or cancellation.

Uploads also have an idle timeout between body chunks (`PP_PHOTO_UPLOAD_BODY_IDLE_TIMEOUT`, 30 seconds by default); an idle upload receives HTTP 408 and its staged file is discarded.

Session configuration uses human-readable durations through the existing configuration parser:

```toml
[session]
idle_timeout = "30d"
secure_cookie = false

[auth]
app_origin = "http://localhost:5173"
```

Equivalent overrides: `PP_SESSION_IDLE_TIMEOUT="12h"` or `--session-idle-timeout "1h 30m"`. Set `PP_SESSION_COOKIE_SECURE=true` or `--session-cookie-secure true` when serving the app over HTTPS; the default is false for local HTTP. The timeout must be at least one second and fit cookie/timestamp arithmetic. Sessions persist across browser restarts. User navigation/interactions renew them with coalescing; passive status requests/background traffic do not. There is no absolute age limit or Remember me checkbox. A timeout change applies to new logins and subsequent renewals. See [ADR 0002](docs/adr/0002-account-sessions.md).

Auth mutations require the configured exact Origin; other/missing origins are rejected, including on login/setup. Browser API calls send their HttpOnly cookie automatically. Credential bodies, hashing concurrency, and sign-in attempts are bounded. The in-process limit defaults to 10 failed credential attempts per peer IP per minute (any rejected login or setup request counts, including malformed or oversized bodies, except `429` refusals and server errors; configurable via the `PP_LIMITER_*` settings; successful sign-ins do not count); behind the current proxy this may be shared by all clients, so one device can still block sign-in with repeated failures, and it resets on API restart. Arbitrary forwarded IP headers are not trusted. At most `PP_AUTH_HASH_CONCURRENCY` (two by default) password hashes run at once and further sign-in or setup requests are refused with 429 without counting as failures, and the limiter tracks at most `PP_LIMITER_MAX_PEERS` peers (new peers are refused while it is full); neither is a substitute for request throttling, which a public deployment should add at an API gateway.

Compose reads its development settings from [.env.example](.env.example)/`.env`: `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB`, `PP_LOG_LEVEL`, `PP_APP_ORIGIN`, `PP_SESSION_IDLE_TIMEOUT`, `PP_SESSION_COOKIE_SECURE`, and the `PP_*_PORT` variables. Changing a published proxy port requires updating `PP_APP_ORIGIN` to match. If database credentials contain URL-special characters, supply a correctly encoded connection URL rather than treating it as plain text. The native Rust TOML configuration is separate; copying it does not configure Compose.

## Verify and build

Choose checks relevant to the change; report checks that could not run. CI definitions live in [.github/workflows/ci.yml](.github/workflows/ci.yml). The release build (`cargo build --workspace --release`) runs only for `v*` release tags and manual runs of [.github/workflows/release.yml](.github/workflows/release.yml), not on pull requests.

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

For a Rust build-output reset, use `cargo clean`. The photo-comment layout regression runs against a disposable shared gallery containing at least
one ready proof, in English. It checks desktop, portrait phone and short landscape viewports:

```sh
PP_E2E_URL=http://localhost:15163 PP_E2E_COMMENTS_TOKEN=<disposable-share-token> pnpm --dir web exec playwright test e2e/comments.spec.ts
```

For a Rust build-output reset, use `cargo clean`. Frontend generated state lives under `web/node_modules`, `web/.svelte-kit`, and `web/build`; reinstall dependencies after removing dependency state. Do not confuse resetting build outputs with deleting persistent database or photo data.

## Contributing

Use [Conventional Commits](https://www.conventionalcommits.org/). Treat changelog maintenance as release work; no automatic changelog-generation workflow is currently configured.

Follow the [shared Superpowers workflow](AGENTS.md#superpowers-delivery-workflow): brainstorm a bounded outcome, approve the written design, plan, implement in an isolated worktree, verify, have fresh code and security reviewers check the change against [REVIEW.md](REVIEW.md), and open a PR that states its risk level. Brainstorming may start without an issue; link an outcome-focused issue before implementation planning. Name branches `<type>/<issue>-slug` with a git-cliff commit type, such as `feat/61-development-workflow`. PRs are squash-merged: medium- and high-risk PRs after explicit approval, low-risk PRs automatically once the auto-merge mode in [AGENTS.md](AGENTS.md#superpowers-delivery-workflow) is `enabled`.

Keep requirements, release acceptance, significant decisions, and implementation details in their respective documents, following [AGENTS.md](AGENTS.md#documentation-and-development-workflow). Preserve committed approved designs. Implementation plans stay untracked in `docs/plans/`; keep lasting decisions in permanent records or the PR. Product requirements and release scope changes require approval.

### Repository settings

Auto-merge is only safe when GitHub enforces the checks. A maintainer configures these by hand:

- **Ruleset** `Protect main` (Settings → Rules → Rulesets): active, targets the default branch, empty bypass list.
  - Require a pull request before merging, with 0 required approvals (agents push with the maintainer's account, which cannot approve its own PRs).
  - Require status checks to pass, and require branches to be up to date before merging. Required checks: `Detect changes`, `Deployment checks`, `Rust format`, `Rust clippy`, `Rust dependencies`, `Rust tests`, `Web format`, `Web lint and check`, `Web dependencies`, `Web tests`, `Web build`. CI skips jobs a PR does not affect, and a skipped job counts as passed. The image builds are not required: their matrix check names differ when skipped, and `docker/` changes are high risk, so they never auto-merge.
  - Block force pushes and restrict deletions.
- **General → Pull Requests**: allow auto-merge.
- **Code security**: enable CodeQL default setup and secret scanning.
