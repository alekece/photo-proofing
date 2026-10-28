# ADR-004: Rust workspace layout and naming

## Status

Accepted for MVP.

## Decision

The Rust code lives in one Cargo workspace under `crates/`, and the SvelteKit frontend lives in `web/`. Crates use the `eg-` prefix ("Enoam Gallery"); folders use plain names.

| Crate | Folder | Responsibility |
| --- | --- | --- |
| `eg-core` | `crates/core` | Shared configuration, error types, and domain types. No I/O, no Actix Web, no database driver. |
| `eg-database` | `crates/database` | Everything database-related: pool, migrations, queries, job queue storage. |
| `eg-api` | `crates/api` | Actix Web API process. |
| `eg-image-worker` | `crates/image-worker` | Image worker process. Does not accept browser requests. |

Dependencies flow one way: `eg-core` <- `eg-database` <- `eg-api` and `eg-image-worker`.

The worker is named `image-worker` because it creates thumbnails, proofs, renditions, and ZIP exports. It is the same codebase as the API, launched as a distinct process (see ADR-003).

Configuration uses `clap` with `clap-config-fallback`. Precedence is command-line flag, environment variable (`EG_*`), optional TOML config file (`--config` or `EG_CONFIG`), then default. `config.example.toml` holds placeholders only; real config files are not committed.

## Consequences

Later work has an obvious home: migrations and queries go in `eg-database`, shared types in `eg-core`. Keeping `eg-core` free of web and database dependencies keeps the worker from pulling in the web stack. Each binary declares its own root CLI struct because the config path field must live on the root type.
