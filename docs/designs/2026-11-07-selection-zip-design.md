# Download the selection as a ZIP

Issue: #69. Requirements: FR-09, NF-01, NF-02, NF-03, NF-04, TC-03, TC-04, AC-02, AC-03. Builds on the
[job queue](2026-10-31-job-queue-design.md), [client selection](2026-11-05-client-selection-design.md) and
[live events](2026-11-04-live-photo-events-design.md). The maintainer authorised implementing without a
separate approval round; every decision below marked **my call** is for the PR review to confirm.

## Intent

From the Favorites tab the photographer presses **Download** and gets one ZIP of the client's selected
originals, exactly as the selection stood when they pressed. Favorites stay editable while it is built.

## Goals

- One action: the ZIP is prepared by the worker, the tab shows Preparing / Ready / Failed, and the browser
  download starts when it is ready.
- Byte-identical originals; membership is the selection at request time.
- Durable and restart-safe; failures and a full disk are visible, never a half-written archive.
- Photographer-only; bounded temporary storage; cleanup.

## Non-goals

Individual or select-mode downloads (FR-09's last sentence stays unavailable here; follow-up issue), a download dashboard, a retouching state, a
selection lock, the client delivery ZIP (#71) and watermarks. The ZIP builder is generic so #71 can reuse it.

## Approaches

1. **Stream the ZIP from the API request.** No state, but it runs inside the request, dies with the
   connection or a restart, cannot show "preparing" and cannot observe a consistent selection. Rejected.
2. **Durable `selection_exports` row plus a worker job that writes a file, then a plain file download
   (chosen).** Reuses the queue (retries, leases, restart recovery) and the event stream, and the download
   is an ordinary static-length response.
3. **Hard-link the originals at request time and build later.** Protects against deletion races, but
   links need a second durable lifecycle for little gain. Rejected.

## Decisions

- **Snapshot (my call).** The request, in one transaction, reads the favorites and stores their ids, names
  and sizes in `selection_export_photos`, with no foreign key to `photos` so a later deletion cannot rewrite
  the snapshot. Later favorite changes never touch it and nothing is locked: the gallery row is only locked
  against a concurrent request.
- **Durable job.** A `selection_exports` row (`preparing`, `ready`, `failed`, with a user-safe
  `failure_reason`) and a `build_selection_export` job are created in the same transaction. The job
  streams each original into a staged file, publishes it atomically and marks the row ready in one
  transaction with completing the job. A crash leaves the job to be reclaimed; a rerun rewrites the same
  deterministic file. Exhausted attempts, including a worker that dies on the last one, fail the export too.
- **One export per gallery in flight (my call).** Requesting while one is preparing returns it; the button
  is disabled meanwhile. A new request after `ready`/`failed` creates a fresh snapshot and removes the
  previous export.
- **Photo removed after the request (my call).** If a snapshot photo is gone when its file is read, or
  when the build is recorded, the export fails with "A selected photo was removed", never a silently
  smaller ZIP, and without retrying; Retry re-snapshots. The recording statement only marks the export
  ready when every snapshot photo still exists. A photo deleted after that moment is the same as one
  deleted after the ZIP was ready: the ZIP is unchanged.
- **Deletion coordination.** Gallery deletion deletes the gallery's export jobs first (in id order, like
  photo jobs) and its export rows cascade, so a late build finds no row: it publishes nothing and removes
  the file it wrote. Replacing an export deletes the old row and its job in one transaction; its file is
  removed after that commit, best effort, with the sweep as the safety net.
- **Exhausted attempts.** `claim` (expired lease on the last attempt), `fail` and `abandon` also mark the
  linked export failed in their own transaction, as they do for a photo, so an export never stays
  Preparing after its job gave up.
- **Library.** The `zip` crate, entries stored uncompressed (JPEGs do not compress, and the size is
  predictable). Writing runs in `spawn_blocking`. The builder lives in `pp-storage` as a reusable
  `ArchiveBuilder` taking `(source key, entry name)` pairs; the selection job is one caller.
- **Safe names.** Entry names come from the stored upload name reduced to its final component, with
  separators, control characters and leading dots removed, capped in length, falling back to `photo.jpg`,
  and de-duplicated (`name.jpg`, `name (2).jpg`, case-insensitively). Entry names are never paths.
  The download filename is fixed and safe (`selection.zip`).
- **Authorization.** Request, status and download are photographer routes under the gallery scope: they
  need the session and the gallery in the caller's workspace, and the download is looked up by export id
  *and* gallery *and* workspace. Client links get no route; the file path comes from UUIDs only. The POST follows the existing origin
  protection of state-changing routes; the download sends `Cache-Control: private, no-store`,
  `X-Content-Type-Options: nosniff` and a fixed attachment filename. The `export` event travels only on
  the photographer's workspace-filtered stream; clients still have none.
- **Bounded storage (my call).** The file lives in the gallery directory, so deleting the gallery removes
  it. At most one retained export per gallery, replaced by the next request. A request is refused (413)
  when the originals total more than `selection-export-max-size` (default 2 GiB, checked from the
  snapshot sizes before anything is written); entries are written with ZIP64 so the limit is a policy,
  not a format limit. An export older than `selection-export-retention` (default 24 h) is treated as gone
  by the API (status 204, download 404) and the worker's startup sweep deletes its row and file, plus
  ZIP files no row owns. There is no workspace-wide quota (followed up): disk use is bounded by one
  capped export per gallery for at most the retention period, plus the concurrency of the worker.
- **Empty selection (my call).** The button is disabled with no favorites, and the server also answers 409
  `empty_selection` for a race, creating nothing.
- **Disk full (my call).** A write error of that kind fails the export at once with "The server is out of disk
  space"; the staged file is dropped. Other I/O errors retry with backoff, then fail visibly.
- **Readiness over SSE.** A trigger announces export status changes on a new channel; the existing stream
  relays them as an `export` event (`gallery_id` only). The tab refetches the authoritative status on that
  event, on `resync` (reconnect) and on load, so a missed event costs nothing. When ready the tab starts
  the download once for the export the user requested; a Ready export found on a later load is offered
  as **Download**, not auto-started.
- **API (my call).** `POST /api/v1/galleries/{id}/selection-export` (202 with the export, 200 if one is
  preparing, 409 empty, 413 too large), `GET` the same path (latest export or 204), and
  `GET .../selection-export/{export_id}/download` (the ZIP as an attachment).
- **UI.** The existing Favorites **Download** button becomes the export action with states per the page
  design: Preparing ZIP (busy, announced politely), Download started, Failed with the reason and Retry.
  Disabled with no favorites.

## Left to the builder

The retention and size defaults, table shape and test fixtures.

## Verification

Exact membership and bytes (unchanged by favorite edits or photo removal after the request), concurrency
of requests, authorization across workspaces/galleries/anonymous, interrupted and retried builds, final
failure and disk-full visible, name safety and de-duplication, cleanup, and the accessible tab states.
