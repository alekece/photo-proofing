# Safe proof thumbnails and previews

Issue: #99 (sub-issue of #66). Requirements: FR-05, FR-06, NF-02, NF-04, AC-02, AC-06; builds on
the [photo storage](2026-10-31-photo-storage-design.md) (#96),
[job queue](2026-10-31-job-queue-design.md) (#97) and
[photo upload](2026-10-31-photo-upload-design.md) (#98) designs.

## Intent

Clients must only ever see reduced, metadata-free derivatives of a proof, never the original.
The worker turns each uploaded original into a thumbnail and a preview that keep the photo's
aspect ratio, orientation and color appearance, and the photographer can see them as soon as they
are ready. Failures are visible and can be retried safely.

## Goals

- A `process_photo` job handler generates a thumbnail and a preview for each photo.
- Derivatives preserve aspect ratio, orientation and color appearance, and carry no source
  identifying metadata, including GPS.
- Originals are never modified; retries never duplicate or corrupt derivatives.
- Failures mark the photo failed with a visible reason, and the photographer can retry.
- The photographer can fetch a ready photo's thumbnail and preview.

## Non-goals

- Serving anything to clients (#101); the upload route and photo list (#98); the UI (#102).
- Watermarks (#67); live updates (#80); quality presets or custom dimensions (FR-06).
- An original-serving route for anyone.

## Anti-goals

- Washed-out or shifted colors because an embedded profile was dropped.
- EXIF, GPS or maker notes surviving in a derivative.
- A late job recreating files for a deleted photo (AC-06).

## Approaches

1. **The `libvips` crate (chosen).** Safe Rust bindings (version 2.3, maintained); one
   `thumbnail` operation can shrink while decoding, apply the orientation, convert through the
   embedded profile to sRGB and strip metadata, with low memory. Needs libvips in the worker
   image and in dev and CI; the decoder runs in the worker process.
2. **The `vipsthumbnail` command as a child process.** Same operations without FFI, and a crash
   or runaway decode only kills the child, which also allows a time limit. Costs a spawn per
   photo, errors through exit codes, and it needs file paths from storage. Kept as the fallback
   if process isolation is wanted later.
3. **Pure Rust (`image`, `moxcms`, `fast_image_resize`).** No native dependency, but orientation,
   profile and metadata handling are written by hand, there is no shrink-on-load, so memory is
   higher, and the color path is less proven. Rejected by the product owner as not needed.

## Decisions

Decided with the product owner:

- Approach 1; a native library in the worker image is accepted.
- Retries are safe and bounded by the job queue (#97).

My calls (to check while reading):

- **Output.** Both derivatives are JPEG, sRGB, rotated to upright pixels with the orientation tag
  removed, and stripped of all metadata. The embedded profile is converted into sRGB rather than
  kept, so no source profile data travels with a proof. Sizes and quality are fixed named values in
  code (about 400 px for the thumbnail and 2000 px for the preview on the long edge, a moderate
  JPEG quality), since FR-06 gives the MVP no preset or dimension controls. The builder tunes the
  numbers.
- **Handler.** It sets the photo `processing`, reads the original through the storage layer
  (#96), writes the thumbnail and preview atomically to `<photo>-thumbnail` and
  `<photo>-preview`, then in one transaction confirms the photo still exists and sets it `ready`.
  A retry rewrites the same keys, so results are never duplicated.
- **Failure.** An error retries with the queue's capped backoff. When attempts run out the photo
  becomes `failed` with a short, human-readable reason (for example "unreadable image"), never
  the raw library error text.
- **Manual retry.** `POST /galleries/{id}/photos/{photo}/retry`, only for a failed photo; it
  resets the photo to `waiting` and requeues its job.
- **Photographer images.** `GET /galleries/{id}/photos/{photo}/thumbnail` and `/preview` are
  workspace-scoped and served only when the photo is `ready`, as `image/jpeg` with
  `X-Content-Type-Options: nosniff` and private caching. There is no route to an original.
- **Resources.** libvips is initialised once, with its internal thread count and operation cache
  capped at startup. Together with the queue's concurrency limit these bound CPU and memory use
  (NF-04), which the library's defaults (all cores, a sizeable cache) would not.
- **Delivery of libvips.** The worker image installs the runtime library and its build stage the
  development package; CI installs it so tests run there.

## Left to the builder

- Confirming the crate exposes the needed operations (thumbnail with auto-rotation, profile
  export, metadata stripping), and the exact sizes, quality and library limits.
- How the original is handed to libvips: a path (which would need the storage layer to expose
  one) or a stream or buffer.
- How the handler learns it is on its final attempt so it can record `failed`, and the exact
  failure reasons.

## Verification

Orientation (all eight EXIF orientations), color (a wide-gamut profile ends up visually
equivalent in sRGB), GPS and EXIF absent from both derivatives, corrupt and truncated input
failing visibly, retry safety and manual retry, worker restart mid-job, the deleted-photo check,
and the thumbnail and preview routes (ready only, cross-gallery and cross-workspace denial, no
original route). The worker image builds and processes a photo under Compose.
