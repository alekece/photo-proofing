# Ready proofs on the shared client page

Issue: #101 (sub-issue of #66). Requirements: FR-04, FR-06, NF-01, AC-01; builds on the
[photo upload](2026-10-31-photo-upload-design.md) (#98) and
[proof derivatives](2026-11-04-proof-derivatives-design.md) (#99) designs and extends the
[private client link](2026-10-30-private-client-link-design.md) (#65).

## Intent

A client holding a valid private link sees the gallery's ready proofs, appearing as the
photographer's uploads finish processing, and can never reach originals, unready or unpublished
files, or another gallery's photos.

## Goals

- One reusable extractor authorises every client route from the link token and the gallery's
  current state.
- Client routes list the ready photos and serve their thumbnail and full-size image. The client
  never names a kind of photo; the API picks the set from the gallery state.
- The client page shows the photos in the gallery's layout, with a viewer, and an empty state.

## Non-goals

- Photographer routes (#98, #99): they need only workspace ownership.
- Finished photos and the delivered state (#70, #71), favorites (#68), downloads, live updates
  (#80) and the archived state (#73); the design leaves a place for each.
- Watermarks (#67).

## Anti-goals

- Any route that returns a proofing original.
- A photo id from another gallery, or an unready photo, being distinguishable from a missing one.

## Approaches

One sensible way: an extractor, in the style of the existing `extractors`, in front of ordinary
routes. Alternatives do not fit. Re-checking the link inside each handler is the drift the
extractor exists to prevent, and exchanging the link for a session cookie adds state the MVP does
not need.

## Decisions

Decided with the product owner:

- The API chooses the photo set from the gallery state; the client never states a kind.
- The photo list is its own route, separate from the gallery response, which also lets a later
  live-update refetch only the list.

My calls (to check while reading):

- **Extractor.** `SharedAccess` takes the `ShareToken` from the path and resolves it to the
  gallery (shared, not expired). #73 adds archived and deleted in the same query, so the rule has
  one home. Unknown, malformed, unshared and expired links are the same 404, as today.
- **Photo set.** A `ClientPhotoSet` value is derived from the gallery state and matched
  exhaustively. This slice has only `Proofs`; #70 and #71 add the finished set, and the compiler
  then forces every client route to handle it.
- **Routes** under `/api/v1/galleries/shared/{token}`:
  - `GET /photos` returns the ready photos as `{id, width, height}`, oldest first, in their
    displayed orientation (see Dimensions).
  - `GET /photos/{id}/thumbnail` and `GET /photos/{id}/image` return the thumbnail and the
    preview for grid and viewer. For proofs these are the generated derivatives, and no route
    reaches an original.
  - Every photo lookup is scoped by the gallery behind the token, so a guessed id from another
    gallery, an unready photo or an unknown id is the same 404.
- **Dimensions.** A photo's stored width and height become its displayed size. Upload already
  reads the JPEG header with libvips, so it also reads the EXIF orientation there and swaps width
  and height for the rotated orientations (5 to 8). The worker changes nothing, and the
  duplicate-upload check stays consistent because a repeated request computes the same values.
  Without this, a sideways-stored portrait would get a landscape tile.
- **Headers.** The list is `no-store`. Images are `image/jpeg` with `private` caching (a CDN or
  shared cache must not hold them), a modest `max-age` since a derivative never changes for its
  id, `nosniff` and no referrer, using the existing response helpers. Revocation cannot recall
  bytes already delivered, which is accepted (the PRD does not promise screenshot protection).
- **Bearer URLs.** Image URLs carry the token in the path because `<img>` cannot send headers.
  The existing rule applies: no access log may record `/galleries/shared/` paths.
- **Client page.** `g/[token]` loads the gallery and the list, renders the grid or masonry
  layout from the dimensions, lazy-loads thumbnails, opens the viewer on the preview image, and
  shows the empty state when nothing is ready. A reload shows newly ready photos; live updates
  arrive with #80. It follows the client page mockups.

## Left to the builder

- Streaming the file to the response, exact `max-age`, and how the viewer handles keyboard and
  swipe navigation within the mockups.
- Query shape and indexes for the gallery-scoped photo lookups.

## Verification

Tests: an upload with a rotated EXIF orientation stores swapped dimensions and one without
keeps them; valid link lists only ready photos and serves their images; pending and failed photos
are absent and 404 by id; another gallery's photo id, unknown ids and a missing derivative are
404; expired and unshared links deny the list and the images; no route serves an original; list
and image headers. Web tests cover the grid, viewer and empty state. Needs #98 and #99 merged.
