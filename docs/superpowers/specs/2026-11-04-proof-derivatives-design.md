# Proof thumbnails and previews

Issue: #99. Requirements: FR-05, FR-06, NF-02, NF-04, TC-03, TC-04 and AC-06 in
[the product specifications](../../SPECIFICATIONS.md).
Foundations: [photo storage](2026-10-31-photo-storage-design.md) (#96),
[job queue](2026-10-31-job-queue-design.md) (#97) and
[photo upload](2026-10-31-photo-upload-design.md) (#98, #132).

## Intent

Clients must only ever see reduced, metadata-free derivatives of a proof, never the original.
After an upload the worker turns the original into a thumbnail and a preview, the photo becomes
`ready`, and the photographer can view both. A photo that cannot be processed becomes visibly
`failed` and can be retried.

## Goals

- One `process_photo` handler in the image worker that produces a thumbnail and a preview with
  correct orientation and color, without source metadata (including GPS).
- `photos.status` is always truthful: `waiting`, `processing`, `ready` or `failed` with a safe
  reason. A photo is never left on `processing` by a crashed or exhausted job.
- Safe retries: a rerun after a crash, lost lease or manual retry never duplicates or corrupts
  derivatives and never modifies the original.
- Manual retry of one failed photo or of every failed photo in a gallery.
- Authenticated, workspace-scoped routes that serve a ready photo's thumbnail and preview.

## Non-goals

- Upload or Photos-tab UI (#102), live updates (#80), the client page and client media routes
  (#101), watermarking (#67) and photo removal or gallery deletion (#68, #73).
- A route that serves originals, any quality-preset or size UI, and a worker-to-API signal.
- Workspace-level size settings: the sizes are constants for now; making them a workspace
  setting later is expected and must not need a redesign.

## Anti-goals

- Any path where a client-reachable route can return an original.
- Raw library or filesystem error text reaching the photographer.
- Two sources of truth that can disagree about whether a photo is processed.

## Approaches

Failure handling decides how `photos.status` stays truthful when the queue gives up on a job.

1. **Handler only.** The handler marks permanent failures and, on its last attempt, marks the
   photo failed. Rejected: a crash or out-of-memory kill never reaches that code, so a
   crash-looping photo stays on `processing` forever.
2. **Handler plus failure update in the job repository (chosen).** The handler decides
   permanent versus transient errors. Whenever a job becomes `failed`, the same SQL statement
   updates the photo, so no path can leave them out of step. The job repository now knows about
   photos, kept behind one `PhotoRepository` function.
3. **Derive status from the job.** Rejected: it makes gallery, dashboard and live-update
   queries join and filter on `jobs`, and photo state would depend on job retention.

A Postgres trigger on `jobs` would also work but hides behaviour from the Rust code and tests.

## Decisions

Decided with the product owner:

- Approach 2. The job repository calls the photo repository, accepting that coupling.
- `photos.status` stays the source of truth, on the `photos` table. A separate status table was
  considered and dropped: it adds a join to every read for a gain that can be added later.
- The `ProcessPhoto` job payload carries a typed `photo_id` in the enum variant itself rather
  than a hand-built JSON value.
- Thumbnail 400 px and preview 1600 px on the long edge, JPEG at about quality 75. The preview
  works on phones and laptops and is too small to be useful as a replacement for the original.
- Manual retry exists per photo and per gallery.

My calls (to check while reading):

- **Pipeline.** libvips (already a dependency). Auto-rotate from the EXIF orientation, convert
  to sRGB and embed no profile so colors render correctly everywhere, strip all metadata
  (EXIF, XMP, IPTC, GPS), never upscale. Both outputs are JPEG. The worker owns its libvips
  initialization like the API does.
- **Outputs.** Written to a temporary file in the same directory, flushed, then renamed to the
  deterministic key `<photo-id>-thumbnail` / `-preview` (existing `Thumbnail` and `Preview`
  variants). A rerun replaces them atomically; a partial file is never visible under a final
  key. The original is opened read-only.
- **State flow.** The photo becomes `processing` when the handler starts and `ready` in the
  transaction that finishes the job. That final update succeeds only if the photo still exists
  and is not already `ready`, so late work after a deletion changes nothing. A restart mid-job
  leaves the lease to expire and the job reruns from the start.
- **Permanent vs transient.** An unreadable or corrupt image, or one over the pixel bound, is
  permanent: the photo becomes `failed` at once with a short fixed reason, the job completes,
  and the queue does not retry. Disk or other infrastructure errors are returned to the queue,
  which retries with backoff; on exhaustion the job repository marks the photo `failed`.
  `failure_reason` is always a short safe message; detailed errors go to logs and the job's
  `last_error` only.
- **Resources.** Reuse the existing pixel bound, decode with sequential access, and cap the
  worker's concurrent photo jobs so memory stays bounded.
- **Retry.** `POST /api/v1/galleries/{id}/photos/{photo_id}/retry` and
  `POST /api/v1/galleries/{id}/photos/retry`. Each, in one transaction, sets failed photos to
  `waiting` and clears the reason, and requeues each one's failed job. Only `failed` photos are
  affected; other states are left alone, so repeated calls are safe. Both require workspace
  ownership; missing and foreign galleries both return 404. The gallery-wide route returns the
  number of photos requeued.
- **Serving.** `GET /api/v1/galleries/{id}/photos/{photo_id}/thumbnail` and `/preview`.
  Session plus workspace ownership. 404 unless the photo belongs to that gallery and is
  `ready`, so cross-gallery, cross-workspace and not-ready cases are indistinguishable. There
  is no original route. Responses are `image/jpeg` with `Cache-Control: private` and an
  `X-Content-Type-Options: nosniff`. File access goes through `PhotoStorage` keys only.

Further calls from the builder check:

- **Payload shape.** Stored jobs are `{"kind": "process_photo", "payload": {"photo_id": "…"}}`.
  The typed variant `ProcessPhoto(ProcessPhoto { photo_id })` serializes to exactly that, so
  existing queued jobs stay readable and no data migration is needed. The failure update reads
  `payload->'payload'->>'photo_id'`; no index is needed at MVP scale.
- **Retry enqueues a fresh job.** A permanent failure completes its job, so a failed photo may
  have no failed job. Retry therefore always enqueues a new `process_photo` job (fresh
  attempts) and deletes any failed job for that photo, in the same transaction as the photo
  reset. Only `failed` photos are touched, so a repeated call cannot enqueue a duplicate.
- **Failure update guard.** The job repository's photo update applies only to photos still
  `waiting` or `processing`. A stale job failing after a retry, or for a `ready` or deleted
  photo, changes nothing.
- **Derivatives replace on rerun.** `StagedPhoto::commit` never replaces an original. The
  storage layer gains replace-on-commit for the `Thumbnail` and `Preview` variants only;
  originals stay write-once. Storage remains local filesystem only (TC-03).
- **Pixel bound.** It currently lives in the API's upload configuration. The worker needs the
  same value, so it moves to a shared place (config or storage crate) with one default; the
  builder picks where.
- **Slow jobs and double runs.** The worker renews leases while a job runs (every third of the
  lease), so a long decode is not reclaimed. If a lease is nevertheless lost and the job runs
  twice, both runs write identical content to the same keys by atomic rename, and the
  `ready` update is guarded, so the outcome is the same.

## Left to the builder

Module layout in the worker, exact constant names and defaults for worker concurrency, the
precise safe-reason wording, the SQL shape of the failure update, and response bodies of the
retry routes beyond the count.

## Verification

Tests cover, using small generated JPEG fixtures: EXIF orientation applied (including a
rotated source), a wide-gamut or non-sRGB source producing a plausible sRGB result, EXIF/GPS
stripped from both outputs, aspect ratio and no upscaling, corrupt and oversize input failing
permanently with a safe reason, an infrastructure error retried and then failing the photo
through the job repository, a crash-looping job ending with a failed photo, a rerun and a
manual retry leaving exactly one intact set of derivatives and an unchanged original, a worker
restart mid-job recovering, late work not recreating a deleted photo, per-photo and gallery
retry (including no effect on non-failed photos), and the serving routes' access rules
(unauthenticated, foreign workspace, foreign gallery, not ready, ready).
