# Changelog

All notable changes to PhotoProofing are documented here.

## Unreleased

### Added

- Initial product, architecture, access lifecycle, job queue, decision, and automation-governance documentation.
- Defined MVP gallery lifecycle, asset visibility, frozen selection snapshots, delivery manifests, and grouped photographer notifications.
- Rust workspace (`pp-core`, `pp-database`, `pp-api`, `pp-image-worker`) with shared clap-based configuration, an API health endpoint, and an idle image worker entrypoint.
- SvelteKit frontend with dashboard and client-gallery route shells.
