# Gallery lifecycle: expiry, archive, restore and delete

Issue: #73. Builds on `2026-11-05-gallery-states-design.md`, `2026-11-04-bulk-photo-delete-design.md` and `2026-10-31-job-queue-design.md`. Product rules: FR-13 and AC-06 in `docs/SPECIFICATIONS.md`.

## Intent

The photographer can end client access without losing anything (expiry, archive), restore it at the same link, and permanently delete a gallery. Deleting never leaves files behind and never lets a running job bring deleted content back.

## Goals

- Archive and restore a gallery. Restore reuses the link, still respects expiry and never changes delivery state.
- Delete a gallery in one atomic step; its files are removed afterwards, surviving crashes and restarts.
- Photographers keep full access to expired and archived galleries, including uploading and editing photos.
- Delete photos and galleries through one cleanup mechanism.

## Non-goals

- Retention-driven deletion, a separate revoke/pause feature, backups, return to proofing.
- Revoking copies a client already downloaded.
- Removing the client event stream (#174). This design assumes it is gone.
- Delete confirmation, button placement and visuals: they follow the approved page design (demo HTML and screenshots). Until then Delete acts immediately.

## Approaches

Archive/restore:
1. **Two actions (chosen):** `POST /galleries/{id}/archive` and `/restore`. They state intent and cannot be combined with other edits.
2. Archive as a field of the existing gallery update. Rejected: hides intent and mixes concerns.

File cleanup:
1. **Cleanup jobs in the existing queue, plus a startup sweep (chosen).** The cleanup job is inserted in the same transaction as the delete, so it cannot be lost; the worker retries it.
2. A periodic timer in the worker or API. Rejected: needs its own scheduler, restart and overlap handling, and gains nothing over the queue.
3. Remove files from the API after the transaction (today's best-effort behaviour). Rejected: a crash between the two leaves orphans.

## Access rules

- A gallery is available to clients only if it is not archived, not expired and not deleted. Client link lookup already enforces archive and expiry; this work adds tests, not a new mechanism.
- Extending or clearing expiry uses the existing gallery update; nothing is regenerated.
- Expiry and archive delete nothing. Photographers keep full access to the gallery, including uploads and edits.
- Direct or stale client requests (page, media, favorites, downloads) are refused the same way, per request. There is no client event stream (see Non-goals), so no live connection needs closing.
- Cross-gallery and cross-workspace attempts keep failing as today.

## Archive and restore

- `archived_at` already exists on `galleries` and nothing sets it. Archive sets it to now; restore clears it. No migration.
- Both are idempotent and scoped to the caller's workspace; an unknown or foreign gallery is a 404.
- Restore does not touch `delivered_at`, `shared_at` or `expires_at`, so a delivered gallery stays delivered and an expired one stays expired.
- Prompting the photographer to update the expiry when the restored gallery is still expired is deferred to #175. Until then restore simply succeeds and the gallery stays expired.

## Delete

- `DELETE /api/v1/galleries/{id}`, scoped to the caller's workspace (404 otherwise).
- One transaction deletes the gallery (photos, jobs and favorites cascade) and inserts a gallery cleanup job naming the gallery. After commit, clients are refused at once.
- Photo deletes (single and bulk) use the same mechanism: the transaction that removes the rows also inserts a cleanup job naming the removed photos. The API stops removing files itself.
- The worker handles the job by removing the files of the named gallery or photos. Removing an already missing file is success, so retries and duplicates are harmless.
- A worker still rendering a deleted photo finds its claim gone and removes what it wrote late (existing behaviour); stale jobs cannot recreate accessible content because nothing serves a file without its photo row.

## Startup sweep

- When the API starts it enqueues one sweep job, unless one is already queued.
- The sweep removes files that have no photo row and are older than a fixed grace period (24 hours, a constant in code, not a setting). The grace period protects uploads whose file exists before its row is committed. It never deletes by a list read earlier; each file is checked against the database when considered.
- No timer and no sweep settings. Strays from crashes are cleaned at the next restart.

## Jobs

- `JobPayload` gains cleanup and sweep variants next to `ProcessPhoto`. Discriminators stay inside the tagged payload JSON, so no schema change.
- Cleanup jobs are bounded: files are removed in batches and a large gallery continues across retries.
- The jobs table has no foreign keys to galleries or photos; payloads carry ids in JSON. A cleanup job therefore survives the cascade, and the photo-delete query that removes a photo's processing jobs matches only `ProcessPhoto` payloads, never cleanup jobs.
- Storage is the local filesystem, laid out `<workspace>/<gallery>/<photo>-<variant>` (ADR 0003), so ids alone locate every file: a cleanup job carries workspace, gallery and, for photo deletes, photo ids. It needs no stored paths.
- Gallery cleanup removes the whole `<workspace>/<gallery>/` directory, so late writes from an upload or render in flight are removed too. Photo cleanup removes all variants (`original`, `thumbnail`, `preview`) of the named photos.
- The sweep reads age from the file's modification time. It only considers photo files under workspace and gallery directories (not the `.tmp` staging area, which storage already owns) and treats a directory whose gallery has no row as wholly orphaned once all its files are past the grace period.

## API and web

- New endpoints: archive, restore, delete (above). The gallery payload already carries `archived_at`/states.
- Web: Archive and Delete buttons at the end of the gallery page's side nav; once archived, Archive is replaced by Restore. Delete leaves for the dashboard. Layout, mobile behaviour and confirmation follow the approved page design.
- The dashboard's Archived and Expired filters already exist and need no change.

## Constraints

- Everything is scoped to the session's workspace.
- Cleanup is safe to run twice and to interrupt.
- Logs and spans carry gallery and photo ids only, never paths with user-supplied text or secrets.

## Verification

Tests for: archive/restore round trip and idempotence; restore keeps delivered and expired state; client lookup refused when archived, expired or deleted, and allowed after restore or extension; delete is atomic with its cleanup job; delete during upload, processing and cleanup; cleanup retries and duplicates; sweep removes only old orphans and keeps files of existing photos and recent files; photo and bulk delete enqueue cleanup; photographer can upload to an archived gallery.

## Left to the builder

Job names and payload fields, batch size, the sweep queue-dedup mechanism, error codes, and web component structure.
