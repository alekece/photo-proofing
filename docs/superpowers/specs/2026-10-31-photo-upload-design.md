# Upload JPEG proofs to a gallery

Issue: #98 (sub-issue of #66). Requirements: FR-05, NF-01, NF-02, NF-04, AC-06; builds on the
[photo storage](2026-10-31-photo-storage-design.md) (#96) and
[job queue](2026-10-31-job-queue-design.md) (#97) designs.

## Intent

The photographer can add photos to a gallery, one or many, and every accepted original is kept
unchanged while bad files are rejected with a clear reason. A returning photographer can list the
photos and see where each one stands.

## Goals

- An authenticated, workspace-scoped route accepts one JPEG per request. A batch is the
  browser sending several requests in parallel; no request carries more than one file.
- The actual JPEG content, size and decoded dimensions are validated, with bounded memory,
  temporary-file and connection use.
- An accepted original is stored unchanged, recorded as a photo, and a processing job is
  enqueued, so accepted work is never lost.
- A route lists all photos of a gallery with their processing status.

## Non-goals

- Thumbnails, previews and the processing itself (#99); serving any image (#99, #101).
- The upload UI (#102); live updates (#80); photo removal (#68).
- Resumable uploads: a failed upload restarts from the beginning (FR-05).
- Duplicate detection of any kind.

## Anti-goals

- A rejected or failed upload leaving a file, a `photos` row or a job behind.
- A photo row without its job, or a job without its stored file, after any crash.

## Approaches

For the request shape:

1. **Raw body, one file per request (chosen).** `Content-Type: image/jpeg`, body is the file,
   original name in a header. No new dependency, streams straight to the staged file, and a
   single file per request holds by construction.
2. **`multipart/form-data` with one file part.** Browser-standard and carries the filename, but
   adds `actix-multipart` and a parser to harden (extra parts, header bounds).
3. **One multipart request for a whole batch.** Rejected: it contradicts per-file parallel
   uploads and makes per-tile progress and Retry hard.

## Decisions

Decided with the product owner:

- Raw body, one file per request; parallel requests form a batch.
- The size, dimension and concurrency limits are configurable settings, not constants.
- Filenames are not checked or rejected. The photographer may upload anything, including many
  files with the same name, since contents can differ.

My calls (to check while reading):

- **Route.** `POST /galleries/{id}/photos` (upload, `201` with the photo) and
  `GET /galleries/{id}/photos` (list, all photos, ordered by creation; the frontend filters).
  A gallery of another workspace is indistinguishable from a missing one.
- **Filename.** Sent percent-encoded in `X-Filename`, stored as given for display and ZIP entry
  names. Only technical handling applies: it must decode to UTF-8 without NUL and fit a length
  cap; otherwise it is stored as a generic name rather than rejecting the upload. It never forms
  a path (#96). ZIP entry names are sanitised when a ZIP is built (#69, #71), because that is
  where a `../` name could harm a client extracting it.
- **Validation order.** `Content-Type` must be `image/jpeg`; a declared `Content-Length` over
  the limit is rejected before reading; the byte count is enforced while streaming (chunked
  bodies have no length); the file must start as a JPEG and its header must parse; the header's
  dimensions are checked against the limit without decoding the image. Rejections are distinct:
  `415` wrong type, `413` too large, `422` malformed JPEG or too many pixels.
- **Limits.** Typed settings that carry their unit (a byte-size type, a pixel-count type, a
  concurrency count), parsed once. Defaults: about 64 MiB per file, about 100 megapixels, and a
  small cap on uploads in flight, beyond which a request waits or is refused with `503`.
- **Accept.** Stream to a staged file, validate, commit the original (#96), then in one
  transaction insert the photo and enqueue its job (#97). If the transaction fails the file is
  removed. A crash between commit and transaction leaves an orphan file, never a visible photo;
  reclaiming such files is left to the cleanup guidance.
- **Photo.** `photos(id, workspace_id, gallery_id, original_name, size, width, height, status,
  failure_reason, created_at)`, `status` a PostgreSQL enum (`waiting`, `processing`, `ready`,
  `failed`) mirrored in Rust. The worker updates `status` in #99; `failure_reason` is filled
  there.
- **Job kind.** This slice adds `process_photo` to `job_kind` in its own migration, after
  #97's. Until #99 registers a handler the worker cannot run it; #98 and #99 should land
  together before a release.
- **Disk full.** A storage-full error maps to a clear failure response, and nothing is left
  behind.

## Left to the builder

- The exact numeric defaults and the JPEG header parser (the same library family as #99's).
- The header name details, error response bodies, and how concurrency is enforced.
- How the staged file handles a client that disconnects mid-body.

## Verification

Validation (non-JPEG, truncated, over-size, over-dimension, wrong type), a batch where one file
fails and the others are kept, a client disconnect leaving nothing, disk-full, duplicate names,
cross-workspace access, and the photo/job pair being committed together. Needs the #96 and #97
foundations merged.

## PR review refinement (2026-11-02)

The following decisions supersede the corresponding implementation calls above; the upload
behavior and storage schema otherwise remain unchanged.

### Approaches

1. **API route owns the transaction.** Rejected because it exposes SQLx transaction details and
   photo/job persistence coordination to the HTTP layer.
2. **`PhotoIntake` coordinates persistence (chosen).** It borrows the pool, creates a transaction
   locally, and calls the focused photo and job repositories with that transaction. This preserves
   the atomic photo/job invariant without implying an uploads table or storing a transaction.
3. **Keep photo and job creation inside `PhotoRepository`.** Rejected because it mixes two
   repositories' responsibilities.

### Decisions

- The API model carries a failure reason in `PhotoStatus::Failed(String)` and removes the
  separate `Photo.failure_reason` field. Serde's externally tagged representation is used:
  successful states remain strings such as `"waiting"`, while a failed state is
  `"status": {"failed": "<reason>"}`. PostgreSQL keeps its `photo_status` enum and nullable
  `failure_reason` column with a constraint requiring a reason exactly when status is failed;
  row decoding combines those columns into the Rust status value.
- Use `NonZeroU64` directly for upload-size limits, pixel limits, and stored photo size. The
  dedicated wrappers added by the initial implementation did not add invariants; field names and
  documentation communicate units.
- Keep `PhotoDimensions`, which groups two non-zero dimensions, and the types that encode actual
  domain variants or distinct error outcomes.
- Rename `PhotoUploadConfig` to `UploadConfig`, rename the module to `upload`, and call its
  shared state `UploadState`. Keep the existing `ApiConfig` field, TOML section, and environment
  variable names unchanged. The associated initializer is `UploadState::install`; the
  non-blocking semaphore method is `try_acquire_slot`.
- Keep upload-pipeline helpers as methods on `UploadState`; Actix route handlers remain free
  functions as required by the route registration pattern. Rename `ApiState::store` to `auth`
  because it returns the authentication repository.
- `PhotoIntake` owns the cross-repository transaction and its uncertain-commit reconciliation.
  The HTTP layer continues to publish the original before database acceptance and only removes it
  after a known failure.
- `PhotoRepository` exposes one transaction-taking insert operation, used by `PhotoIntake`.
  `JobRepository` keeps `enqueue` for standalone enqueueing and `enqueue_in` for a caller-owned
  transaction; both have real call sites. Do not add a generic database-or-transaction executor
  abstraction for this flow.

## Second PR review refinement (2026-11-02, approved)

This refinement addresses the next review of #132. It preserves the upload contract and database
schema except for the explicitly accepted overload-response ordering below. It supersedes the
initializer, state-access, and staging ownership decisions in the first refinement.

### Approaches

1. **Acquire a typed slot inside the handler.** Preserves gallery/header validation before
   admission, but leaves acquisition as a manual handler step.
2. **Eager `UploadSlot` extractor (chosen by the maintainer).** Makes upload admission and the
   permit lifetime explicit in the handler signature. A saturated server can return `503` before
   gallery/header validation; this tradeoff is accepted. Returning or cancelling the request drops
   the slot and releases its permit.
3. **Keep the untyped permit and separate upload application data.** Retains current behavior,
   but does not address the requested slot abstraction and shared-state simplification.

### Decisions and implementation

- Keep the module named `upload` (maintainer decision): its responsibilities extend beyond image
  initialization.
- Expose a free `upload::install()` initializer for bounded, process-wide image-library setup.
  Keep resource construction separate from that initialization; no public image module is added.
- Put shared upload resources inside `ApiState`, with only `Data<ApiState>` registered as
  application state. The semaphore remains shared across workers.
- Add an `UploadSlot` extractor owning an acquired permit and access to the shared upload
  resources. Staging is a method on this type, so staging cannot begin without admission.
  Keep the slot alive through validation, publication, database acceptance, and cleanup.
- Preserve authentication, gallery isolation, body limits, and fail-fast admission. Validate the
  eager-acquisition tradeoff with tests: invalid requests release their slots, and overload can
  precede handler validation. Do not consume a body before authorization.
- Replace repository-specific `ApiState` getters with `database()`. Callers select the required
  repository through the existing `Database::repository` API; persistence stays outside HTTP.
- Explain the `Ok(None)` cleanup branch: the gallery disappeared or no longer belongs to the
  workspace when insertion rechecked it. Rename failure cleanup to `cleanup_after_failure`.
- Keep one Rust photo-status enum, `PhotoStatus`, including `Failed(String)`. Decode the
  PostgreSQL enum as text at the SQL boundary and combine it with `failure_reason`, rejecting
  unknown statuses and inconsistent combinations. Keep the PostgreSQL enum and constraint.
- Rename commit-error classification to `from_commit`, and put the severity-specific decision
  on a private extension trait for `PgSeverity`. An unavailable severity remains uncertain;
  fatal and panic errors must not permit removal of a possibly committed original.
- Rename the retrying intake entry point to `try_accept` and the private, single-transaction
  operation to `accept`. Keep the retry bounded and preserve atomicity and reconciliation.
- Apply the requested blank-line correction. Update related documentation and regression tests;
  do not change unrelated repository APIs or add dependencies.

### Verification

Cover eager slot acquisition, saturation, release after validation failure and cancellation, and
holding a slot through a stalled request. Retain JPEG rejection, timeout cleanup, gallery-deletion,
photo/job atomicity, failure-reason decoding, and uncertain-commit preservation regressions.
Run formatting, relevant Clippy checks, and workspace tests, then obtain independent review before
replying to the new threads with the `[agent]` prefix.
