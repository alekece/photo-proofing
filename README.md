# PhotoProofing

A self-hosted workspace for a photographer to share private photo proofs, collect client favorites, export a selection for external retouching, and deliver finished photos through the same gallery link.

The planned MVP is a local-network workflow trial with replaceable JPEG photos, one photographer account/workspace, and responsive photographer/client screens. Billing is outside the product's scope.

## Project status and documentation

Development is restarting around the [product specifications](docs/SPECIFICATIONS.md) and [MVP scope and acceptance checklist](docs/releases/MVP.md). These documents describe the target product, not completed features.

The repository currently contains an Actix API health endpoint, configuration/logging support, an idle worker process, a database crate placeholder, notification domain code, a dashboard shell, and a mock-backed client gallery/viewer. Authentication, persistent galleries, the durable queue, actual uploads/processing, and delivery are not complete. SQLx, Tailwind, shadcn-svelte, and SSE are agreed implementation choices, not installed features.

- [Product specifications](docs/SPECIFICATIONS.md): requirements, constraints, and exclusions.
- [MVP release](docs/releases/MVP.md): first-release scope and acceptance evidence to collect.
- [ADR 0001 — Technology baseline](docs/adr/0001-technology-baseline.md): established choices and tradeoffs.
- [Agent guidance](AGENTS.md): shared development and documentation rules.
- [Approved design records](docs/superpowers/specs/): designs retained with the repository.

## Prerequisites

For local development:

- A current stable Rust toolchain supporting edition 2024.
- Nightly rustfmt for the repository's import-grouping configuration: `rustup toolchain install nightly --component rustfmt`.
- Node.js 24 and pnpm 10.13.1, matching the frontend CI setup. With Corepack available: `corepack enable` and `corepack prepare pnpm@10.13.1 --activate`.
- PostgreSQL 17 when running database-backed work/tests; the current database crate is a placeholder. Compose supplies a development PostgreSQL service.

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

Open `http://localhost:8000`. The proxy forwards `/api` to the API and other paths to the frontend. Check the API health endpoint:

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

Defaults: API at `http://127.0.0.1:8080`, frontend at `http://localhost:5173`. The frontend proxies `/api` to the API. The dashboard is at `/dashboard`; `/g/<gallery>` currently uses mock gallery data, not uploaded client galleries.

```sh
curl http://127.0.0.1:8080/api/v1/health
```

No API database connection or migrations are wired up by merely starting the current placeholder database crate. Do not use real client photos or credentials to evaluate the mock scaffold.

## Configuration

Rust process settings use this precedence: CLI flags, environment variables, optional TOML file, defaults. Copy [config.example.toml](config.example.toml) to the ignored `config.toml` if needed. For example:

```sh
cargo run -p pp-api -- --config config.toml
```

API settings include `PP_HOST`, `PP_PORT`, and `PP_LOG_LEVEL`; both Rust processes accept `PP_CONFIG` and logging configuration. Use `--help` for process-specific flags. Frontend development proxy target: `PP_API_PROXY_TARGET` (default `http://127.0.0.1:8080`).

Compose reads its development settings from [.env.example](.env.example)/`.env`: `POSTGRES_USER`, `POSTGRES_PASSWORD`, `POSTGRES_DB`, `PP_LOG_LEVEL`, and the `PP_*_PORT` variables. The native Rust TOML configuration is separate; copying it does not configure Compose.

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

Database-backed tests introduced later should use a throwaway database through `DATABASE_URL`; CI already provisions PostgreSQL 17 for Rust tests. For example, after creating a dedicated test database:

```sh
export DATABASE_URL=postgres://USER:PASSWORD@localhost:5432/pp_test
```

This variable is not currently a complete application database setup procedure. Keep test fixtures synthetic and keep original trial photos elsewhere. Backup/restore tooling is excluded from MVP, but persistence across normal restarts is required.

For a Rust build-output reset, use `cargo clean`. Frontend generated state lives under `web/node_modules`, `web/.svelte-kit`, and `web/build`; reinstall dependencies after removing dependency state. Do not confuse resetting build outputs with deleting persistent database or photo data.

## Contributing

Use [Conventional Commits](https://www.conventionalcommits.org/). Treat changelog maintenance as release work; no automatic changelog-generation workflow is currently configured.

Start changes from a product journey and bounded user-visible scope. Keep requirements, release acceptance, significant decisions, and implementation details in their respective documents, following [AGENTS.md](AGENTS.md#documentation-and-development-workflow). Preserve approved designs. Implementation plans remain during PR review and are removed before merge after lasting decisions have been retained in permanent records.
