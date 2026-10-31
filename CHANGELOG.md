# Changelog

All notable changes to PhotoProofing are documented here.

## Unreleased

### Added

- Initial product, architecture, access lifecycle, job queue, decision, and automation-governance documentation.
- Defined MVP gallery lifecycle, asset visibility, frozen selection snapshots, delivery manifests, and grouped photographer notifications.
- Rust workspace (`pp-core`, `pp-database`, `pp-api`, `pp-image-worker`) with shared clap-based configuration, an API health endpoint, and an idle image worker entrypoint.
- Durable PostgreSQL job queue (`jobs` table, `JobRepository`) run by the image worker with leases, bounded retries with backoff, bounded concurrency and graceful shutdown; the worker now needs `PP_DATABASE_URL` (#97).
- SvelteKit frontend with dashboard and client-gallery route shells.
