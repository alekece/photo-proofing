# Finals draft design

Issue: [#70](https://github.com/alekece/photo-proofing/issues/70). Product requirements: FR-05, FR-06, FR-10, NF-01–NF-04, TC-03–TC-05, AC-01, AC-02, AC-06 in `docs/SPECIFICATIONS.md`. Builds on the photo upload, proof derivatives, bulk delete and UI redesign designs. Written and implemented in one pass on the maintainer's authorisation; the PR is the review gate. Choices the maintainer did not make are marked **my call**.

## Intent

The photographer uploads the finished JPEGs independently of the proofing selection, watches them become ready, and removes the ones that do not belong. Nothing here is visible to the client. Deliver, the published set and client downloads are [#71](https://github.com/alekece/photo-proofing/issues/71).

## Goals and non-goals

Goals:

- Finished photos uploaded one by one or in batches, with the proof upload's bounds, storage, status model, retry and live updates.
- Uploaded bytes preserved exactly; a private thumbnail and preview for review.
- Photographer-only review and removal; no client route reaches a finished photo.
- A draft model that #71 can publish from without rework, and that keeps the files of an already published set when the draft changes.

Non-goals: Deliver/publish, client pages and downloads, ZIPs, matching finals to proofs, retouching, watermarks on finals.

## Constraints

- Local storage keys are `workspace/gallery/photo/variant`; photo ids are random UUIDs, so ids of the two sets never collide.
- Anything that must survive a restart lives in PostgreSQL; multi-step changes run in one transaction (`crates/AGENTS.md`).
- Existing client routes and counts must keep seeing proofs only.

## Approaches

1. **Separate `final_photos` table** with its own repository, intake, retry, deletion, job kind, events, handler and routes. Strongest separation, but duplicates about a thousand lines and every future fix to the proof pipeline.
2. **One `photos` table with a `kind` column** (`proof` or `final`). The pipeline (intake transaction, job, worker, retry, deletion, cleanup, sweep, SSE) is shared. Isolation comes from making the kind an explicit parameter: every `PhotoRepository` query that reads a gallery's photos (`list`, `get`, `list_ready`, `is_ready`, `set_favorite`, `favorites`) takes a `PhotoKind`, as bulk deletion and retry already do. A repository opened for one kind would silently serve proofs to a caller that forgot to pick; a required parameter cannot be forgotten, a client route passes `proof` where it reads, and the kind-agnostic job helpers stay off any kind-scoped type.
3. **Finals as a gallery-like child entity.** More structure than this slice or #71 needs.

**Recommendation: approach 2.** (my call) Finals have their own lifecycle columns and API, and nothing exposes them to proof code paths, which is what "separate from proofs" must guarantee. The tests assert it (a final id on a client route, a client count or a favorite is a 404 or ignored).

## Decisions

- **Data model.** `photos.kind photo_kind NOT NULL DEFAULT 'proof'`. Two nullable timestamps, only valid for finals: `published_at` (set by #71 when a final enters the delivered set) and `removed_at` (the photographer removed it from the draft). The draft is the finals with `removed_at IS NULL`. The published set (#71) is the finals with `published_at IS NOT NULL`. Check constraints forbid both columns on proofs and `removed_at` without `published_at`.
- **Removal.** A final never published is deleted with its jobs and its files are cleaned up, exactly like a proof. A published final is only marked `removed_at`: it leaves the draft but its row and files stay until #71 next publishes. Nothing in this slice sets `published_at`; tests set it directly to prove the retention. (my call; the issue's "later explicit publication" wins over the PRD sentence that finals reach clients without a publish step. If the PRD wins, #71 simply publishes on every change.)
- **Reuse.** Upload intake, size/pixel bounds, upload slots, job `process_photo`, retry and the 1000-id bulk delete are used unchanged; the kind is a parameter. The worker locates a photo without a kind and renders the same thumbnail and preview, so they stay generic derivatives (watermarks, #67, only affect proofs).
- **Byte and metadata preservation.** Same as proofs: the staged upload is committed as the `original` variant without re-encoding; derivatives are separate files. No route serves the original; the finals bytes reach clients only through #71's download.
- **Review thumbnails.** Served to the authenticated photographer only through `GET /galleries/{id}/photos/{photo_id}/{thumbnail|preview}?finals=true` for ready drafts; waiting, failed and removed finals are 404.
- **API.** The photo routes gain an optional `finals=true` query parameter (default `false`: proofs), e.g. `/galleries/{id}/photos?finals=true`, `…/photos/{photo}?finals=true`, `…/photos/delete?finals=true`, `…/photos/retry?finals=true`; an unreadable value is a 400. Same handlers, authorisation, limits and responses. Each is scoped by workspace, gallery and kind, so a proof id with `finals=true` (or the reverse) is 404.
- **Authorisation.** Photographer routes need a session and gallery ownership. Client routes always read the proof kind: a finals id on a client media or favorite route is 404/ignored, the client page lists proofs only, and gallery counts, quota, hero and favorites count proofs only. There is no client event stream.
- **SSE and reconnect.** `photo_events` payloads gain `kind`; removal from the draft also emits `deleted`. The dashboard keeps one panel per set, ignores the other set's events, and refetches on resync, restoring readiness after a reconnect or restart.
- **Interrupted work.** The upload queue carries the set; accepted files live in storage and the table, jobs are durable, failed files stay and are retryable, and a failed upload leaves no partial file (the existing rules).
- **UI (Final tab).** `PhotosPanel` takes a `set`; the Final tab uses it with the same grid, select mode, status chips, retry, viewer, drop zone and empty state "Drop the finished photos here". Hero, favorite and watermark affordances are proof-only. The placeholder `FinalPanel` is removed. The "downloaded by the client" tile marker stays for #71.
- **Migration.** `0013_finals_draft.sql` (renumber if another PR lands first).

## Isolation and lifecycle details (from design review)

- **Kind-agnostic by design:** `locate`, the job-completion helpers (`mark_ready_in`, `mark_failed_in`, `exists_in`), the cleanup handler and the orphan sweep work by photo id for both kinds; the worker therefore processes finals, and the sweep never treats a final's file as an orphan.
- **Raw gallery SQL** that reads `photos` directly gets `kind = 'proof'`: dashboard counts, hero eligibility and the shared-page hero lookup.
- **Removal SQL.** In one transaction, the processing jobs of the named finals are deleted; unpublished rows are deleted (and only their ids get a cleanup job); published rows get `removed_at = now()` if not already set. A repeated removal of a removed final is a no-op and is not reported. `list`, `get`, `list_ready`, retry, thumbnails and previews all exclude `removed_at IS NOT NULL`.
- **Events.** The notify trigger also fires on `UPDATE OF removed_at` and reports `deleted`.
- **Retention.** A removed published row keeps its files until #71's next publish removes it and queues cleanup; no cleanup job names a published id before that. Deleting the gallery removes everything, overriding retention. #71 must publish only ready finals.
- **Spec conflict.** FR-10/AC-04 (no publish step) and the issue (explicit publication) differ. This slice follows the issue; if the PRD wins, #71 publishes on every change and the two columns still fit.
- **Slots.** Upload slots and bounds are server-wide, as for proofs, not per gallery or kind. Finals and proofs share them; a very large proof batch only queues finals behind the same slots.

## Verification

Database tests for kind scoping, soft removal, the CHECKs and event payload; API tests for finals upload, listing, independence from proofs, malformed and over-limit files, byte preservation, retry, deletion, cross-gallery/kind/workspace and client denial; worker test for finals; web tests for the API client, queue and panel.
