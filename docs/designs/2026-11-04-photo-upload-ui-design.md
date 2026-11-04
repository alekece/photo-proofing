# Photos tab: upload queue, processing status and single-photo delete

Issue: #102. Requirements: FR-05, FR-06, NF-02, NF-03, NF-04, TC-03, TC-04; builds on the [photo
upload](2026-10-31-photo-upload-design.md) (#98), [proof
derivatives](2026-11-04-proof-derivatives-design.md) (#99) and [live photo
events](2026-11-04-live-photo-events-design.md) (#80) designs, and on the [gallery page
mockup](assets/pages/gallery.html). Second layer of a stack with #80 below and #146 (select mode,
bulk delete) above.

## Intent

The photographer drops a batch of JPEGs on the Photos tab and immediately sees one tile per file.
The tiles move through upload, processing and ready, when the thumbnail appears, while the
photographer can keep working elsewhere in the app. Anything that went wrong says so and offers
the right way out, and any photo can be deleted at any moment.

## Goals

- Drop or pick files; a tile appears for each at once.
- An app-wide upload queue that keeps sending while the photographer moves between dashboard
  pages, sending only as many at once as the server allows.
- Tiles show each state clearly, live, through #80's events.
- Retry for recoverable failures, ✕ for everything, a confirmation before deleting a photo that
  reached the server.
- A delete-photo API that works whatever the photo's status, without letting late processing
  bring the photo or its files back.
- 50–100 photo batches stay smooth.

## Non-goals

- Upload progress percentages.
- Selecting several photos and deleting them together (#146).
- Favorites and the warning when deleting a favorited photo (#68).
- Background sweep of stray files (#73); finals and the Finals tab (#70).
- Surviving a tab close or reload: unsent files are lost then.

## Anti-goals

- A batch that fails because the server was briefly busy.
- A deleted photo reappearing, or its files staying reachable.
- Asking for confirmation to drop a file that never reached the server.

## Approaches

1. **App-wide queue in the dashboard layout, live status from #80 (chosen).** The queue lives
   above the pages, so in-app navigation does not stop it, and tiles change on events instead of
   polling. Cost: needs #80 first, hence the stack.
2. **Queue inside the Photos tab, short polling while photos are pending.** Simpler, but leaving
   the page stops uploads, which the product owner rejected, and polling is what the ADR avoids.
3. **Background Service Worker uploads.** Would survive closing the tab, but `File` handles and
   cookies in service workers add a lot of complexity for a case the product owner accepted to
   lose.

## Decisions

Decided with the product owner:

- Dropped files appear at once as tiles. Only a limited number upload at the same time; the limit
  is the server's configurable `photo_upload.concurrency`, never hard-coded in the browser.
- Uploads continue when the photographer leaves the gallery page and returns. Closing or
  reloading the tab cancels unsent files, with the browser's "Leave site?" warning while any are
  pending. Logging out stops the queue.
- One looping (indeterminate) bar, no percentage, used for both Uploading and Processing; only the
  label changes. Waiting states show a still bar.
- Tile states and actions, as in the validated demo:

  | State | Label | Bar | Actions |
  |---|---|---|---|
  | Waiting to upload | "Waiting to upload" | still | ✕ |
  | Uploading | "Uploading" | looping | ✕ (cancels) |
  | Upload failed (recoverable) | red "Upload failed" | none | Retry, ✕ |
  | Rejected (not a JPEG, too large, unreadable) | red reason | none | ✕ |
  | Waiting to process | "Waiting to process" | still | ✕ |
  | Processing | "Processing" | looping | ✕ |
  | Processing failed | red "Processing failed" | none | Retry, ✕ |
  | Ready | thumbnail | none | ✕ |

- ✕ sits top right and appears on hover or keyboard focus (always visible on touch screens).
  Retry is a round floating button at the bottom right, always visible.
- Any photo can be deleted at any moment, whatever its status. A photo already on the server asks
  for confirmation first; a file still in the browser's queue goes away at once.
- Select mode and bulk deletion are a separate issue (#146).

My calls (to check while reading):

- **Upload limits route.** `GET /api/v1/upload/limits` (session required) returns
  `{concurrency, max_size, max_pixels}` from the API config. The browser sends at most
  `concurrency` files at a time and rejects files over `max_size` before sending ("Too large").
  Other dashboard users share the same server slots, so the server can still be busy.
- **Busy is not broken.** Today every 503 looks alike. The upload route now answers **429** with
  `Retry-After` when no upload slot is free, and **507** when photo storage is full; other 503s stay
  server errors. On 429 the queue puts the file back to wait and retries after the given delay,
  and only after several in a row shows "Upload failed" with Retry. On 507 that file and every
  file still waiting in the queue show "Storage full" with Retry, so a full disk does not receive
  the rest of the batch.
- **Answer mapping.** 201: the tile now follows the server photo. 413, 415, 422: rejected with a
  short reason. 429 and 507: as above. Other 5xx and network errors: "Upload failed" with Retry.
  404: the gallery is gone, so the queue drops that gallery's files. 401: the session ended; the
  queue stops and the page goes to login.
- **Queue store.** `lib/gallery/uploads.svelte.ts`, a runes class created by the dashboard layout
  and passed through context: items `{key, galleryId, file, state, photoId?, error?}`, one
  `AbortController` per upload, `beforeunload` while anything is queued or uploading, `clear()` on
  logout. Files are checked client-side by type or `.jpg`/`.jpeg` extension; the server stays
  authoritative.
- **Photos tab.** A `PhotosPanel` component replaces the placeholder: loads `GET
  /galleries/{id}/photos`, merges it with the queue's items for that gallery (server photos in their
  order, then queued files in drop order), and applies #80's `photo` events by id. An event for an
  unknown id while this gallery still has uploads in flight is usually one of them saved before its
  201 arrived, so it is ignored and the 201 brings it in; otherwise, and on `resync`, the list is
  refetched (at most once a second). When the gallery's uploads finish, the list is refetched once
  to catch any photo whose 201 was lost. The whole panel is a drop target; the empty state is the
  drop zone from the mockup; `+ Upload` opens a multi-file picker (`accept="image/jpeg"`). Ready
  tiles open the existing `ProofViewer` on the photographer preview route. Thumbnails lazy-load;
  tiles are keyed by photo id or queue key so a 100-file batch does not re-render everything.
- **Delete route.** `DELETE /api/v1/galleries/{id}/photos/{photo_id}` (session and ownership),
  204 on success, 404 when there is no such photo in this workspace and gallery. One transaction
  deletes the photo's jobs and the photo row; after commit, the original, thumbnail and preview
  are removed, a missing file being fine and any other storage error logged (#73's sweep is the
  backstop). The worker already handles the race: deleting its running job makes its claim
  stale, and `complete_photo` then reports `Missing`, so it removes what it just wrote. The
  `photos` trigger from #80 announces the deletion to every open page.
- **Cancelling an upload.** ✕ on an uploading tile aborts the request without confirmation. If
  the server already answered 201, the queue deletes the new photo. If the abort races a commit the
  browser never hears about, the photo appears on the next event or refresh and can be deleted
  like any other.
- **Delete in the browser.** The tile disappears immediately (optimistic); a failed request other
  than 404 puts it back with an error toast. The confirmation uses the existing `Dialog`.
- **Accessibility.** Tiles are list items with their state as text; ✕ and Retry are buttons with
  labels that name the file; the drop zone has a keyboard-reachable `+ Upload`; state changes are
  announced through a polite live region (count-based, not one message per file).

## Left to the builder

- Number of busy retries and their delays; toast wording; exact tile markup and spacing within the
  mockup tokens.
- Whether `ProofViewer` gets a small adapter or a prop for the photographer URLs.
- Where `+ Upload` and the hidden file input live within `GalleryTabs`.

## Verification

API tests: limits route needs a session and returns the config; no free slot answers 429 with
`Retry-After` and full storage answers 507; delete removes the row, its jobs and its files for each
status; delete during processing leaves no derivative once the worker finishes; another workspace or
gallery gets 404; the trigger emits `deleted`. Web tests: dropping files creates tiles; at most
`concurrency` uploads run; 429 requeues; 507 stops the batch; each answer maps to its state; ✕ on
queued, uploading and server tiles (with confirmation only for server ones); Retry on both failure
kinds; events move tiles to ready and remove deleted ones; `beforeunload` only while pending; the
queue survives navigating away from and back to the gallery page; axe passes on the panel.
