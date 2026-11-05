# Client selection

Issue: #68.

## Goal

The client marks favorites through the private link; every browser using the link shares one persistent selection, and the photographer sees it in the gallery's Favorites tab.

## Client behaviour

- A heart sits at the top right of each photo, in the grid and in the viewer: shown on hover and keyboard focus, always on touch screens, and always on a favorite. One click toggles the favorite and saves at once. There is no confirmation message and no submit button.
- The header shows the count, for example "13 selected", and with an included allowance "13 selected · 10 included · 3 extra". Without an allowance, selection is unlimited. Overflow is allowed: when the selection exceeds the allowance, the count is shown in bold in the warning colour (not red, since nothing is wrong). Reaching the allowance changes nothing else. The colour is never the only signal: the "extra" count stays in the text.
- A failed save reverts the heart and shows a short error; the next click retries.
- Once the photographer delivers, the hearts disappear and the selection is read-only.
- Other browsers on the same link see changes live. After a connection loss they refresh the whole selection on reconnect, so the server's list is authoritative.

## Photographer behaviour

- The existing **Favorites** tab shows only the client's selection, read-only, with "n of N included" when configured. Export and Download stay with the export issue.
- Deleting a photo that is a favorite asks for confirmation and removes only that favorite with the photo; other choices stay.

## API and storage

- Table `favorites(gallery_id, photo_id, created_at)` with primary key `(gallery_id, photo_id)`; `photo_id` references `photos(id) ON DELETE CASCADE`.
- Client routes under `/shared/{token}`:
  - `GET .../favorites` returns the favorite photo ids.
  - `PUT .../favorites/{photo_id}` adds and `DELETE` removes. Both are idempotent, so repeated or concurrent requests converge and cannot corrupt the selection.
- Server checks: the share token must be valid (not expired, not archived); the photo must be a ready photo of that gallery; writes are rejected with a conflict once `delivered_at` is set. (Needs the `delivered_at` column from #86; the delivery slice must also reject favorite writes at its publication boundary.)
- Photographer route: `GET /api/v1/galleries/{id}/favorites`, scoped to the workspace.
- Changes publish through the same Postgres notification mechanism as the photo events, to a per-link event stream for clients and to the photographer's existing stream, which refresh the list.

## Web

- `ProofThumbnail` and `ProofViewer` get the heart; `ClientGallery` owns the selection state and counts. Controls are shared by both layouts and are disabled in the owner preview (#81).
- `GalleryTabs` replaces the Favorites placeholder with the read-only list.

## Tests

Persistence across visits, repeated and concurrent toggles, overflow counts, deletion of a favorite photo, cross-gallery and not-ready photos rejected, expired, archived and delivered links rejected, failed-write revert, live refresh and reconnect refresh, keyboard and accessible names (`aria-pressed`).
