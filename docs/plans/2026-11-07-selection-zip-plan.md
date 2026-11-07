# Selection ZIP plan (#69)

Design: docs/designs/2026-11-07-selection-zip-design.md. Test first in each step.

1. Migration `0013_selection_exports` (tables, enum, notify trigger, job kind needs none: payload is JSON).
2. `pp-database`: `selection_export` repository (request snapshot, get latest, mark ready/failed, replace),
   `BuildSelectionExport` payload, jobs.rs failure hook, deletion.rs job cleanup, `ExportEvent` channel.
3. `pp-storage`: `ArchiveKey`, `ArchiveBuilder` (zip crate, safe unique names, ZIP64), commit/read/remove, sweep.
4. Worker: `SelectionExportHandler`, registration, retention in sweep.
5. API: config, routes (POST/GET/download), SSE `export` event.
6. Web: schemas, api client, events, FavoritesPanel export button states + tests.
7. Docs (README status), checks, review, remove this plan.
