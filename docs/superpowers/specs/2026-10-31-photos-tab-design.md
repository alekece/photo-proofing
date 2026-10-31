# Photos tab: upload and track processing

Issue: #102 (sub-issue of #66). Requirements: FR-05, FR-06, NF-03, NF-04; builds on the
[photo upload](2026-10-31-photo-upload-design.md) (#98), [photo processing](2026-10-31-photo-processing-design.md)
(#99) and [client proofs](2026-10-31-client-proofs-design.md) (#101) designs and the
[page designs](2026-10-30-page-designs-design.md).

## Intent

The photographer adds photos from the gallery page, can leave, and on return sees which photos are
ready, still processing or failed, and can retry what failed. The page matches the Photos tab
mockup (`gallery.html`, `prototype.html`).

## Goals

- `+ Upload` and the empty-state drop zone add JPEGs, one request per file, a bounded number in
  parallel.
- Upload transfer progress is shown separately from waiting, processing, ready and failed.
- Ready photos show as thumbnails in the gallery's layout and open in the viewer; failed ones show
  a reason and a Retry.
- Status refreshes when the photographer returns or reloads.
- Fully keyboard-operable on mobile and desktop, with visible focus and meaningful status text.
- A gallery of 50 to 100 photos stays operable.

## Non-goals

- Live status updates (#80), hero choice (#85), removing photos (#68), photo ordering.
- The Favorites and Finals tabs, which stay placeholders.
- Resuming a partial upload: a failed upload restarts from the beginning (FR-05).

## Anti-goals

- Status carried by color alone.
- A tile claiming "ready" or "uploaded" before the server says so.
- Periodic polling of the server (ADR 0001); a throwaway timer that #80 would delete.

## Approaches

One sensible way: extend the existing Photos tab and the proof grid and viewer, with an upload
manager that outlives the page. Alternatives do not fit: `fetch` cannot report upload progress, so
`XMLHttpRequest` is required for the raw-body upload; and polling for status is ruled out by the
ADR, with SSE following in #80.

## Decisions

Decided with the product owner:

- One request per file, in parallel; keep it simple.
- No polling: processing status refreshes when the photographer returns or reloads, until #80.

My calls (to check while reading):

- **Data.** The gallery page loads the photo list from the #98 route in its server load, like the
  gallery itself. Thumbnails come from the #99 photographer route and the viewer shows the
  preview.
- **Tiles.** Ready: thumbnail, opens the viewer. Waiting or processing: a placeholder with status
  text. Failed processing: the reason and a Retry that calls the #99 retry route. A file still
  being transferred: a placeholder with a progress bar, separate from the status wording.
- **Upload manager.** An app-level store, not page state, owns the queue, so progress survives
  leaving the gallery page and returning inside the app. It sends at most three uploads at once.
  A failed transfer shows the server's reason (too large, not a JPEG and so on) and a Retry that
  resends the same file. That file is only held in memory, so after a reload the transfer failure
  is gone, while failed processing, which lives on the server, stays retryable.
- **After a transfer.** The list is refetched once when a file finishes, so the new photo appears
  as waiting. Nothing else updates until the photographer returns or reloads.
- **Leaving.** While transfers are running, closing or reloading the tab triggers the browser's
  leave warning, because that kills the in-flight requests.
- **No client-side checks.** The server is the authority for type, size and dimensions; the page
  just shows its reason.
- **Share.** Following the page design, Share is disabled until at least one photo is ready. This
  is a UI guard only: the server still allows sharing an empty gallery, and the PRD's empty-gallery
  warning (FR-04) is superseded by the page design pending the PRD update in PR #88.
- **Accessibility.** The upload button and drop zone are real controls with a labelled file input
  (`multiple`, JPEG only). A polite live region announces progress and failures ("3 of 12
  uploaded, 1 failed"). Each tile states its status in text, progress uses a native progress
  element, and Retry is a normal keyboard-reachable button.
- **Scale.** Thumbnails load lazily; the grid is not virtualised unless a 100-photo test shows it
  is needed.

## Left to the builder

- The exact store shape, how queued, uploading and failed items are merged with server photos in
  one list, and the tile layout details within the mockups.
- Retry wording and the exact messages for the server's failure reasons.

## Verification

Component tests with a mocked `XMLHttpRequest`: parallel limit, progress, failure with Retry,
refetch after completion, and the tile states. Accessibility checks for the live region, labels
and focus. A test with 100 photos for render cost, and a manual check on a phone-sized viewport.
Needs #98, #99 and #101 merged.
