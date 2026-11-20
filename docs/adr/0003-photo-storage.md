# ADR 0003 — Private photo storage layout

Status: Accepted through the approved [photo-storage design](../designs/2026-10-31-photo-storage-design.md), issue [#96](https://github.com/alekece/photo-proofing/issues/96).

## Context

Accepted photos are the photographer's only copy of their originals. The API (uploads, serving) and the worker (previews) must share one private, durable place for them. [ADR 0001](0001-technology-baseline.md) selects the local filesystem; this records the layout, which is costly to change once real photos exist.

## Decision

Photos live under `PP_PHOTO_DIR` (`/var/lib/pp/photos` in Compose, a named volume `photo-data` mounted in `api` and `worker`). Originals and unversioned derivatives use `<workspace-id>/<gallery-id>/<photo-id>-<variant>`, with UUID identifiers and fixed variant names. There is no extension: the format is validated at upload and a derivative's format may change. A finished photo is stored as the `original` of its own photo id.

The `pp-storage` crate owns all paths and file operations. A `PhotoKey` uses UUIDs and a `Variant` enum, so no user-supplied string reaches a path. Writes are staged in `<photo-dir>/.tmp`, synced, then installed with an atomic `rename` (derivatives) or `hard_link` (originals, which are written once and never silently replaced). Dropping an uncommitted write removes the temp file. Disk-full is a distinct error. Startup fails if the directory is missing or not writable. Dockerfiles create the directory owned by `pp` so a fresh volume is writable.

Every marked proof publication attempt uses its own UUID in
`<photo-id>-watermarks/<generation>-<variant>`. The worker syncs both files before committing
the generation in PostgreSQL; the API selects only that pair. Successful publication removes
only the pair it replaced. If a reader selected a just-removed pair, the API reauthorizes
and reselects the same marked variant once. Failed or unconfirmed attempts can leave
unreferenced files; photo deletion removes the whole marked directory. Legacy rows without
a generation still use the flat marked filenames.

## Alternatives and consequences

Original filenames as paths were rejected: they invite traversal and collisions; names live only in the database. A module in `pp-core` or a storage trait with one implementation were rejected for cohesion and TC-03.

Files are not browsable and the volume is not a usable backup without the database, because names carry no meaning. Backup is deferred by the PRD, so this is accepted. Crash leftovers in `.tmp` are never published but are not yet purged. Gallery deletion and orphan cleanup are not part of this layer (#73). Moving to object storage later would need an adapter and a data migration.
