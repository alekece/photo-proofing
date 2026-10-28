# Enoam Gallery Specifications

## Current status

The current planned release is the MVP, tracked in [specs/MVP.md](specs/MVP.md). It is a self-hosted proofing and delivery workspace for one photographer and their clients.

## Product principles

1. The photographer controls every gallery, share link, asset, and delivery decision.
2. Private sources are never accessible from a client gallery.
3. Client proofs are for selection, not print-quality delivery.
4. Delivery exposes only explicitly published retouched final files.
5. Security rules are enforced by the backend, never by hidden browser controls alone.
6. A simple, reliable workflow is more valuable than a freeform gallery builder in the first release.

## Documents

- [MVP scope](specs/MVP.md): user journeys, acceptance criteria, scope boundaries.
- [System architecture](architecture/SYSTEM.md): Rust, SvelteKit, PostgreSQL, storage, deployment.
- [Access and asset lifecycle](architecture/ACCESS_AND_ASSET_LIFECYCLE.md): gallery states and exact visibility rules.
- [Job queue](architecture/JOB_QUEUE.md): asynchronous image, export, and retry processing.
- [Automation governance](AUTOMATION_GOVERNANCE.md): safe repository checks, implementation, and merge rules.
- [Decision records](decisions/): durable technical and product decisions.

## Versioning policy

The MVP specification is the single current source of truth while the first release is being built. Do not maintain parallel documents such as `SPECS-MVP.md` and `SPECS-1.0.md`, as they will drift.

Once a release is approved, freeze a dated release snapshot in `docs/specs/`, record the release in `CHANGELOG.md`, and tag the corresponding Git commit. Treat the application as version `0.x` until the gallery state model, delivery manifest, migration process, and backup and restore procedure are stable enough for `1.0`.

## Glossary

- **Original source**: the JPEG or RAW file uploaded by the photographer.
- **Proof**: a deliberately restricted preview used for client selection.
- **Final**: a photographer-published retouched delivery file.
- **Delivery manifest**: the frozen list of final assets a client is permitted to see and download.
- **Selection snapshot**: the timestamped set of favourites frozen by the photographer when retouching starts.
- **Share link**: an opaque, unguessable link that grants limited gallery access after server-side validation.