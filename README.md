# enoam-gallery

Self-hosted private proofing and delivery workspace for one photographer. See [docs/architecture/SYSTEM.md](docs/architecture/SYSTEM.md) and [docs/decisions/ADR-004-workspace-layout-and-naming.md](docs/decisions/ADR-004-workspace-layout-and-naming.md).

## Local development

Prerequisites: a recent stable Rust toolchain, Node.js, and pnpm. Docker Compose, PostgreSQL, and CI are not set up yet.

Run each process in its own terminal:

```sh
cargo run -p eg-api                # API on 127.0.0.1:8080
cargo run -p eg-image-worker       # image worker (idle until job queue work lands)
pnpm --dir web install && pnpm --dir web dev   # frontend on http://localhost:5173
```

Check the API: `curl localhost:8080/api/v1/health` returns `{"status":"ok"}`. The frontend dev server proxies `/api` to the API. Routes: `/dashboard` and `/g/<gallery>`.

Checks:

```sh
cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
pnpm --dir web check && pnpm --dir web test && pnpm --dir web build
```

### Configuration

Settings come from, in order of precedence: command-line flag, environment variable (`EG_HOST`, `EG_PORT`, `EG_LOG_LEVEL`), optional TOML file, default. Copy `config.example.toml` to `config.toml` (gitignored) and pass it with `--config config.toml` or `EG_CONFIG`. Never commit real secrets.