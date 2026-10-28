# Contributing

Every command below is run from the repository root and is the same one CI runs (`.github/workflows/ci.yml`; Rust and web jobs run only when their own files change).

## Prerequisites

- Stable Rust toolchain (edition 2024) and a **nightly** toolchain with `rustfmt`: `rustup toolchain install nightly --component rustfmt`
- Node.js 24 and pnpm 10.13.1 (`corepack enable && corepack prepare pnpm@10.13.1 --activate`)
- PostgreSQL 17 for database-backed tests (see [Tests](#tests))

## First run

```sh
pnpm --dir web install --frozen-lockfile
cp config.example.toml config.toml   # optional, gitignored; never commit real secrets
```

## Start and stop

Run each process in its own terminal; stop it with `Ctrl-C`.

```sh
cargo run -p eg-api                # API on 127.0.0.1:8080
cargo run -p eg-image-worker       # image worker
pnpm --dir web dev                 # frontend on http://localhost:5173
```

Verify: `curl localhost:8080/api/v1/health` returns `{"status":"ok"}`.

Docker Compose start/stop commands: coming with #25.

## Reset

```sh
cargo clean                                          # Rust build output
rm -rf web/node_modules web/.svelte-kit web/build    # frontend state
pnpm --dir web install --frozen-lockfile
```

Resetting the database and local storage: coming with #14 (migrations) and #25 (Docker Compose).

## Tests

```sh
cargo test --workspace
pnpm --dir web test
```

Tests that need PostgreSQL read `DATABASE_URL` and fail when it is unset. Point it at a throwaway local database, for example:

```sh
export DATABASE_URL=postgres://USER:PASSWORD@localhost:5432/eg_test
```

CI provides a Postgres 17 service container and sets `DATABASE_URL` for the test job. Test fixtures must be synthetic: never use client photos, names, or other client assets.

## Lint

```sh
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir web check
```

## Formatting

```sh
cargo +nightly fmt --check   # verify
cargo +nightly fmt           # apply
```

Nightly rustfmt is required: `rustfmt.toml` sets `group_imports = "StdExternalCrate"`, which stable rustfmt does not support. Stable `cargo fmt` would ignore the import grouping.

## Build

```sh
cargo build --workspace --release
pnpm --dir web build
```

## Commits

Use [Conventional Commits](https://www.conventionalcommits.org/). `CHANGELOG.md` is generated at release; do not edit it.
