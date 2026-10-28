# enoam-gallery

Self-hosted private proofing and delivery workspace for one photographer. See [docs/architecture/SYSTEM.md](docs/architecture/SYSTEM.md) and [docs/decisions/ADR-004-workspace-layout-and-naming.md](docs/decisions/ADR-004-workspace-layout-and-naming.md).

## Local development

Prerequisites: a recent stable Rust toolchain, Node.js, and pnpm. Docker Compose and PostgreSQL are not set up yet (coming with #25 and #14). Full command reference (start, stop, reset, test, lint, formatting, build): [CONTRIBUTING.md](CONTRIBUTING.md). CI runs the same checks.

Run each process in its own terminal:

```sh
cargo run -p eg-api                # API on 127.0.0.1:8080
cargo run -p eg-image-worker       # image worker (idle until job queue work lands)
pnpm --dir web install && pnpm --dir web dev   # frontend on http://localhost:5173
```

Check the API: `curl localhost:8080/api/v1/health` returns `{"status":"ok"}`. The frontend dev server proxies `/api` to the API. Routes: `/dashboard` and `/g/<gallery>`.

Checks (import grouping needs nightly rustfmt):

```sh
cargo +nightly fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm --dir web check && pnpm --dir web test && pnpm --dir web build
```

### Docker Compose

Prerequisite: Docker Compose. Start the whole stack (PostgreSQL, API, worker, frontend, Caddy proxy) with one command:

```sh
cp .env.example .env   # local-only sample values; .env is gitignored
docker compose up --build --wait
```

`--wait` returns once every service reports healthy; `docker compose ps` shows readiness. Open http://localhost:8000 (proxy: `/api` goes to the API, everything else to the frontend) and check `curl localhost:8000/api/v1/health`. Host ports are set in `.env` (`EG_PROXY_PORT`, `EG_API_HOST_PORT`, `EG_WEB_HOST_PORT`, `EG_POSTGRES_HOST_PORT`). Stop with `docker compose down` (add `-v` to drop the database volume).

Health checks: API polls `/api/v1/health`, web fetches `/`, PostgreSQL uses `pg_isready`, the proxy serves `/proxy-health`, and the worker (no HTTP listener) is healthy while `eg-image-worker` is PID 1. The frontend image runs the Vite dev server, since there is no production adapter yet.

### Configuration

Settings come from, in order of precedence: command-line flag, environment variable (`EG_HOST`, `EG_PORT`, `EG_LOG_LEVEL`), optional TOML file, default. Copy `config.example.toml` to `config.toml` (gitignored) and pass it with `--config config.toml` or `EG_CONFIG`. Never commit real secrets.