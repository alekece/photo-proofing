# PhotoProofing

A self-hosted workspace for a photographer to share private photo proofs, collect client favorites, export a selection for external retouching, and deliver finished photos through the same gallery link.

The planned MVP is a local-network workflow trial with replaceable JPEG photos, one photographer account/workspace, and responsive photographer/client screens. Billing is outside the product's scope.

## Project status and documentation

Development is restarting around the [product specifications](docs/SPECIFICATIONS.md) and [MVP scope and acceptance checklist](docs/releases/MVP.md). These documents describe the target product, not completed features.

The repository supports one-time photographer account setup, email/password login, a protected dashboard, persistent revocable sessions, and persistent galleries with autosaved settings (title, layout, expiry, included photo count, watermark flag) backed by SQLx/PostgreSQL. Auth screens use Tailwind and selected shadcn-svelte components. Configuration/logging, API health, an idle worker, in-memory notification domain code, and a mock-backed client gallery/viewer also exist. Photo upload and the hero picker, the durable queue, uploads/processing, delivery, and SSE are not implemented.

- [Product specifications](docs/SPECIFICATIONS.md): requirements, constraints, and exclusions.
- [MVP release](docs/releases/MVP.md): first-release scope and acceptance evidence to collect.
- [ADR 0001 — Technology baseline](docs/adr/0001-technology-baseline.md): established choices and tradeoffs.
- [Agent guidance](AGENTS.md): shared development and documentation rules.
- [Approved design records](docs/superpowers/specs/): designs retained with the repository.

## Prerequisites

For local development:

- A current stable Rust toolchain supporting edition 2024.
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

Expected response: `{"status":"ok"}`. The web container serves the Vite development server; there is no production deployment adapter configured. Health checks establish process/readiness checks, not the complete proofing workflow.

The current Compose host ports bind to `127.0.0.1`. Other devices on the local network cannot reach them using these defaults. LAN configuration is a later deployment implementation task; these instructions do not change bindings or expose the service publicly.

Stop the stack with:

```sh
docker compose down
```

This preserves the named PostgreSQL volume. `docker compose down -v` deletes that volume and its database contents. The current stack does not yet configure persistent photo storage; that is required before implementing uploads.

### Native development

Start a local PostgreSQL 17 instance first, or start only the Compose database with `docker compose up -d postgres`. Native defaults use the sample account/database from `.env.example` at port 5432. Set `PP_DATABASE_URL` when using another database or port. The API connects and applies embedded migrations before listening; it fails startup if the database is unavailable.

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

Defaults: API at `http://127.0.0.1:8080`, frontend at `http://localhost:5173`. The frontend proxies `/api` to the API. The dashboard, which lists and creates galleries, is at `/dashboard`; `/g/<gallery>` currently uses mock gallery data, not uploaded client galleries.

```sh
curl http://127.0.0.1:8080/api/v1/health
```

Open `http://localhost:5173/login` for native setup/sign-in. Account setup closes after the first account, including concurrent attempts, but until then anyone who can reach the API can create it: run setup before exposing the host to other devices. Sign out revokes that particular session, including any copy of its cookie; independent sessions remain signed in. The current HTTP trial sends passwords and cookies unencrypted: use a trusted network and synthetic credentials that you do not reuse. Do not use real client photos.

## Configuration

Rust process settings use this precedence: CLI flags, environment variables, optional TOML file, defaults. Copy [config.example.toml](config.example.toml) to the ignored `config.toml` if needed. For example:

```sh
cargo run -p pp-api -- --config config.toml
```

API settings include `PP_HOST`, `PP_PORT`, `PP_LOG_LEVEL`, `PP_DATABASE_URL`, `PP_APP_ORIGIN`, `PP_SESSION_IDLE_TIMEOUT`, `PP_SESSION_COOKIE_NAME` (default `pp_session`), and the sign-in limiter settings `PP_LIMITER_MAX_FAILURES` (default 10), `PP_LIMITER_WINDOW` (default `1m`) and `PP_LIMITER_MAX_PEERS` (default 1024); both Rust processes accept `PP_CONFIG` and logging configuration. Use `--help` for process-specific flags. Frontend development proxy target: `PP_API_PROXY_TARGET` (default `http://127.0.0.1:8080`). Server-side auth checks use `PP_API_INTERNAL_URL` with the same native default; Compose sets it to `http://api:8080`. These frontend settings use its Node runtime, not the Rust TOML file.

Session configuration uses human-readable durations through the existing configuration parser:

```toml
[session]
idle_timeout = "30d"

[auth]
app_origin = "http://localhost:5173"
```

Equivalent overrides: `PP_SESSION_IDLE_TIMEOUT="12h"` or `--session-idle-timeout "1h 30m"`. The timeout must be at least one second and fit cookie/timestamp arithmetic. Sessions persist across browser restarts. User navigation/interactions renew them with coalescing; passive status requests/background traffic do not. There is no absolute age limit or Remember me checkbox. A timeout change applies to new logins and subsequent renewals. See [ADR 0002](docs/adr/0002-account-sessions.md).

Auth mutations require the configured exact Origin; other/missing origins are rejected, including on login/setup. Browser API calls send their HttpOnly cookie automatically. Credential bodies, hashing concurrency, and sign-in attempts are bounded. The in-process limit defaults to 10 failed credential attempts per peer IP per minute (configurable via the `PP_LIMITER_*` settings; successful sign-ins do not count); behind the current proxy this may be shared by all clients, so one device can still block sign-in with repeated failures, and it resets on API restart. Arbitrary forwarded IP headers are not trusted. At most two password hashes run at once and further sign-in or setup requests are refused with 429 without counting as failures, and the limiter tracks at most `PP_LIMITER_MAX_PEERS` peers (new peers are refused while it is full); neither is a substitute for request throttling, which a public deployment should add at an API gateway.

Compose reads its development settings from [.env.example](.env.example)/`.env`: `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB`, `PP_LOG_LEVEL`, `PP_APP_ORIGIN`, `PP_SESSION_IDLE_TIMEOUT`, and the `PP_*_PORT` variables. Changing a published proxy port requires updating `PP_APP_ORIGIN` to match. If database credentials contain URL-special characters, supply a correctly encoded connection URL rather than treating it as plain text. The native Rust TOML configuration is separate; copying it does not configure Compose.

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
pnpm --dir web check
pnpm --dir web test
pnpm --dir web build
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

Follow the [shared Superpowers workflow](AGENTS.md#superpowers-delivery-workflow): brainstorm a bounded outcome, approve the written design, plan, implement in an isolated worktree, verify, request fresh independent review, and open a PR. Brainstorming may start without an issue; link an outcome-focused issue before implementation planning. Use Git Flow branch types with issue-prefixed names, such as `feature/61-development-workflow`. Merge only after explicit approval, preferably with a squash merge.

Keep requirements, release acceptance, significant decisions, and implementation details in their respective documents, following [AGENTS.md](AGENTS.md#documentation-and-development-workflow). Preserve committed approved designs. Commit plans on the working branch and retain them through PR review and corrections, then remove them before final merge approval after retaining lasting decisions in permanent records. Product requirements and release scope changes require approval.
