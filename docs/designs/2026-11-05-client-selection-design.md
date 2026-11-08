# Client selection

Issue: #68.

## Goal

The client marks favorites through the private link; every browser using the link shares one persistent selection, and the photographer sees it in the gallery's Favorites tab.

## Client behaviour

- A heart sits at the top right of each photo, in the grid and in the viewer: shown on hover and keyboard focus, always on touch screens, and always on a favorite. One click toggles the favorite and saves at once. There is no confirmation message and no submit button.
- The header shows the count, for example "13 selected", and with an included allowance "13 of 10", without a separate extra count. Without an allowance, selection is unlimited. Overflow is allowed: when the selection exceeds the allowance, the count is shown in bold in the warning colour (not red, since nothing is wrong). Reaching the allowance changes nothing else. The colour is never the only signal: "13 of 10" already says the selection is over.
- A failed save reverts the heart and shows a short error; the next click retries.
- Once the photographer delivers, the hearts disappear and the selection is read-only.
- Clients have no live updates. A browser keeps its own selection until reload; a fresh load shows the server's list.

## Photographer behaviour

- The existing **Favorites** tab shows only the client's selection, read-only, with "n of N included" when configured. Export and Download stay with the export issue.
- Deleting a photo that is a favorite asks for confirmation and removes only that favorite with the photo; other choices stay.

## API and storage

- `photos.favorite boolean NOT NULL DEFAULT false`. The selection is one per gallery by design (no per-person selections, so no identities or reconciliation), which makes a column enough: a deleted photo takes its favorite with it, and a favorite can never point at another gallery. The Favorites tab uses the same order as the Photos tab.
- Client routes under `/shared/{token}`:
  - `GET .../favorites` returns the favorite photo ids.
  - `PUT .../favorites/{photo_id}` adds and `DELETE` removes. Both are single conditional `UPDATE`s on the photo, so repeated or concurrent requests converge and cannot corrupt the selection.
- Server checks: the share token must be valid (not expired, not archived); the photo must be a ready photo of that gallery; writes are rejected with a conflict once `delivered_at` is set. (Needs the `delivered_at` column from #86; the delivery slice must also reject favorite writes at its publication boundary.)
- Photographer side: no separate route. The workspace-scoped photo list already carries each photo's `favorite` flag, and the Favorites tab filters it.
- Toggling a favorite does not change `status`, so the existing photo-event triggers stay silent. Changes publish through a separate Postgres notification, in the same way as the photo events, to the photographer's existing stream, which refreshes the list.

## Web

- `ProofThumbnail` and `ProofViewer` get the heart; `ClientGallery` owns the selection state and counts. Controls are shared by both layouts and are disabled in the owner preview (#81).
- `GalleryTabs` replaces the Favorites placeholder with the read-only list.

## Tests

Persistence across visits, repeated and concurrent toggles, overflow counts, deletion of a favorite photo, cross-gallery and not-ready photos rejected, expired, archived and delivered links rejected, failed-write revert, the photographer's live refresh, keyboard and accessible names (`aria-pressed`).
