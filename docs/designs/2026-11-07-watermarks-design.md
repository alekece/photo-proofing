# Workspace watermark on proofs

Issue: #67. Requirements: FR-02, FR-03, FR-05 to FR-07, NF-01 to NF-04 and TC-05 in
[the product specifications](../SPECIFICATIONS.md).
Builds on [proof derivatives](2026-11-04-proof-derivatives-design.md),
[gallery configuration](2026-10-30-gallery-configuration-design.md) and
[live photo events](2026-11-04-live-photo-events-design.md). The Settings screen is in
[the UI design](ui/screens/settings.md); its prototype and components already exist.

## Intent

The photographer saves one watermark for the workspace (studio name, logo, or both), and each
gallery switches it on or off. Every proof has clean and marked thumbnail/preview pairs;
only client-gallery presentation uses the switch to choose between them. Photographer images
and the client hero stay clean. Only proofs are
marked: a gallery's finals and ZIP exports of originals never carry the watermark, and changing
the settings leaves them alone. Changing the watermark
rebuilds marked copies across the workspace without exposing an original or a stale marked
copy. Toggling a gallery never rebuilds images. See
[instant switching and clean hero](2026-11-07-watermark-variants-design.md) for the approved revision.

## Goals

- Persisted workspace settings: studio name, logo, mark (name, logo, both), pattern (repeat or
  once), position (3 × 3), size (five steps) and cover signature, edited on the Settings page.
- A logo validated independently of JPEG photo uploads, with bounded bytes, pixels and memory.
- Per-gallery switch (already stored as `watermark_enabled`) that now changes the output.
- Regeneration after a relevant change, with truthful waiting, failed and retry states, safe
  against changes while photos are processing and against restarts and crashes.
- Everything survives restarts and container recreation.

## Non-goals

- Per-gallery watermark designs, fonts, colours or opacity controls (excluded by the PRD).
- Changing the existing client cover signature: keep its overlay separate from the clean hero.
- Applying the watermark to originals or finished photos, which stay unchanged.
- Rotating the repeated mark or other speculative rendering settings.

## Constraints

- Client proof access requires a valid clean pair and, when enabled, the current marked
  revision. The dedicated cover route serves only the current hero's clean preview.
  No route or fallback serves a proof original.
- Every multi-step change is one database transaction; anything that must survive a restart
  lives in the database. The image worker owns all libvips rendering of photos.

## Approaches

How a change reaches the already generated derivatives:

1. **Keep serving stale marked copies.** Rejected: they do not apply the saved watermark.
2. **Prepare both pairs, hide only stale marked copies (chosen).** Upload prepares all four
   files. A switch selects existing files with no jobs. Settings changes queue marked updates;
   enabled clients wait for the current revision, while clean owner images, disabled clients
   and clean covers remain available. Reuses states, events, retry and failure reasons.
3. **Render on demand and cache.** Rejected: puts CPU-heavy libvips work and the watermark
   decision on the request path and needs a cache invalidation scheme.

How a render that overlaps a change is kept from publishing stale files:

1. **Version the files** by watermark revision. Rejected: changes the storage key scheme.
2. **Check, then write** with a guard in the final update. Rejected: a slow older job can still
   overwrite newer files before its guard fails.
3. **Publish under the photo's row lock (chosen).** See "Publication".

## Decisions (my call unless noted)

**Settings storage.** A `workspace_settings` row per workspace holds the fields above, the
normalised logo PNG as `bytea` (at most a few hundred KB, so it lives and is backed up with the
database and survives container recreation without a new volume), and `watermark_revision`; it cascades with the workspace.
No row means defaults: empty name, mark `name`, repeat, position 8, size 2, signature `name`,
which is no watermark yet.

**Effective watermark and revision.** The *watermark* is what actually gets drawn: the text
(when the mark uses the name and it is not empty) and the logo (when the mark uses it), plus
pattern, size and, only for `once`, position. `watermark_revision` increases exactly when the
saved watermark differs from the previous one, so saving the cover signature or an unused logo
regenerates nothing. A mark that uses the name needs a non-empty name and one that uses the logo
needs a logo; otherwise the save is rejected with a field-level 422 (the save transaction checks it against the stored logo). Name: 1 to 100 characters after trimming, or empty when the mark is the logo.

**Logo.** PNG or JPEG, the formats the app already accepts (transparency matters, so the stored copy is always a PNG; no SVG attack surface, no extra codecs). Limits, fixed
constants (my call: one logo per studio does not warrant deployment knobs): 1 MiB upload and
4 megapixels. The API accepts it
inside the settings request as base64, checks the PNG or JPEG signature, rejects it on header dimensions before decoding pixels, then decodes in a blocking task and
re-encodes a copy with its long edge at most 512 px and all metadata dropped. Failures are 422
with a short safe reason (not a PNG or JPEG, too large, too many pixels, unreadable). The request body
limit of that route fits a base64 logo at the size limit.

**API.** `GET /api/v1/settings` returns the fields and whether a logo exists;
`GET /api/v1/settings/logo` returns the stored logo, a PNG (session required, `private, no-store`,
`nosniff`); `PUT /api/v1/settings` saves everything in one request, with `logo` set to `keep`,
`remove` or a new base64 PNG or JPEG, so a name, a logo and a mark change together or not at all.
Everything is workspace-scoped and needs a session.

**Rendering.** The worker prepares/resizes once, then encodes clean and marked pairs from the
same pixels. With no effective watermark the marked files reuse clean encoded bytes. Marking
happens after resizing and before encoding, so the
mark is relative to the output: its width is a share of the image width (10, 16, 24, 34 or 48 %
for steps 0 to 4). Text uses the system sans bold font, white at reduced opacity over a soft
dark shadow so it reads on light and dark photos; the logo is drawn at the same opacity; "both"
stacks the logo above the name. `once` places it in the chosen cell of the 3 × 3 grid with a
margin; `repeat` tiles it over the whole image in rows offset by half a cell. The worker image
installs `fonts-dejavu-core`. Orientation, sRGB conversion and metadata stripping are unchanged,
and the original is only read.

**Regeneration.** One repository transaction does the change and, for every affected `ready` or
`processing` photo, sets it `waiting` and inserts a `process_photo` job (one bulk statement, so
thousands of photos stay one short transaction). Affected means every proof in the workspace,
including disabled galleries, when the effective revision increases. Switches change only the
gallery row. Published clean readiness survives marked updates and failures. Lock order is
the workspace row first, then settings and photos in id order; inserting jobs locks nothing else, and the
regeneration never deletes or updates existing jobs, so it cannot deadlock with the worker
(which locks job, then photo). `waiting` photos need no job: their queued job reads the settings
after it marks the photo `processing`. The regeneration scan locks waiting proofs too,
without resetting them or adding jobs: a worker starting after its row was scanned waits for
the settings commit and reads the new revision. A worker that started before its row was
locked is seen as processing and requeued. `failed` photos use the current settings when retried. A stale job that later exhausts
its attempts on an infrastructure error fails its photo like any other job; the photographer
retries it. No new job kind and no change to the job payload.

Photo intake and manual retry take a shared foreign-key lock on the workspace before
inserting or reactivating photos; settings
saves hold its exclusive lock through regeneration. This prevents new rows from escaping
the scan and starting with uncommitted old settings, without changing upload/job contents.

**Publication.** The handler marks the photo `processing`, then reads clean readiness and
settings (their *stamp*: workspace revision, independent of the switch), renders, and stages
all four initial files or only the marked pair on subsequent updates as temporary
files. It then opens a publication: one transaction that locks the job row, then the photo row,
and checks that the claim is still current (job `running` with the same attempt), the photo still
exists, and the stamp read under the lock equals the one the render used. If any check fails
nothing is renamed and the job completes; for a stale stamp the change that made it stale
has already requeued the photo (it was `processing` when that change committed). Otherwise only
the selected same-filesystem renames happen inside the lock, then clean readiness and published
marked revision are recorded, the photo is marked `ready`, and
the job completed in that same transaction. A change cannot overtake it (its photo update waits
for the lock), so an older render can never overwrite newer files and a `ready` photo always
matches the current settings. If the commit fails after the renames, initial files are not
declared clean and updated marked files are not declared current; retry renders again.

**States and SSE.** The existing photo triggers announce each `waiting` and `ready` change over
the photographer's event stream, so the Photos tab shows "Updating watermarks…" and refreshes on
reconnect with no new event type. Failures show the existing safe reason and the retry
actions. After a save that regenerates, Settings shows the existing toast.

**Never an original.** There is no fallback from missing or stale marked copies to clean
copies when enabled. A missing or unreadable original fails processing with the existing
reasons. Already-published clean copies remain usable during marked-only failures.

**Serving and upgrade.** Authenticated owner media is clean unless the caller asks for
`?variant=watermarked`, as the client preview does; public client routes pick the variant
from the saved switch. Client cover access is scoped to
the current permitted hero. Proof responses are private/no-store. Existing proof files might
already be marked: migration defaults clean readiness to false and durably queues one-time
preparation from originals for ready/processing proofs. Failed proofs retain manual retry.
Finals are untouched. Stop old API/workers before migration and never resume old two-file
workers afterward; preserve database and photo volumes.

**Web.** `/settings` loads the saved settings server-side, the logo tile shows
`/api/v1/settings/logo`, Save sends one `PUT`, and server validation messages appear inline and
in a toast. The toolbar and previews exist already; the preview is a client-side sketch, not
the worker's output.

## Left to the builder

Module layout, exact opacity and shadow constants, SQL shape, and wording of validation messages.

## Verification

Tests cover: settings round trip, defaults, revision bump only on effective change, validation
(mark without name or logo, long name, bad position or size); logos that are neither PNG nor JPEG, truncated,
oversized in bytes or pixels, plus a valid transparent PNG and a JPEG normalised and stripped; rendering of
text, logo, both, repeat and once at several positions (pixels differ from the plain render,
dimensions, orientation and metadata unchanged); gallery switches creating no jobs or resets
and settings changes regenerating proofs in all galleries; a change during processing leaving no stale `ready`
photo; a crash or restart between change and rendering; a failed or inaccessible original never
becoming ready; client routes denying stale marked copies while clean access stays usable;
clean hero/owner media, upgrade and all-variant cleanup; and the web page's load, save,
validation and logo flows.
