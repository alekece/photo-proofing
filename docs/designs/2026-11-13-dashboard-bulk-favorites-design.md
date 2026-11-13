# Bulk gallery selection, favorites, and quick edit on the dashboard

Issue: [#346](https://github.com/alekece/photo-proofing/issues/346). Builds on `GalleryCard` inline access icons (#344) and `GalleryStudio` (#342), touching `crates/database`, `crates/api`, `web/`, `docs/SPECIFICATIONS.md`, and `docs/releases/MVP.md`.

## Intent

Photographers managing dozens of galleries need a fast way to pin active or priority galleries to the top of the dashboard and batch-edit titles, shoot dates, or deletions without opening each gallery individually.

## Goals

- **Gallery favorites (`favorite: boolean`):**
  - Favorited galleries sort **first** (`favorite DESC`) ahead of every `GallerySort` mode (`updated`, `created`, `oldest`, `title`).
  - Favorited `GalleryCard`s display a subtle warm gold **star icon** (`Star` filled `#f2c063`) inline beside the gallery `<h2>` title (next to the #344 `Lock` and `EyeOff` icons).
  - Photographers can toggle `favorite` both from the dashboard floating bulk bar (`Favorite` / `Unfavorite`) and via a subtle star toggle button inside the studio gallery rail (`GalleryStudio`).
- **Dashboard card selection:**
  - Hovering over a `GalleryCard` reveals a `26 px` translucent top-left selection circle (also visible on touch viewports and whenever 1+ galleries are selected); clicking toggles selection without navigating, highlighting selected cards with a crisp Warm Paper / ink ring and solid check disc.
  - Interactive elements are never nested: the selection `<button>` sits as a sibling of the card `<a>` inside the `<li>` (`vitest-axe` verified).
  - Pressing `Escape` clears the selection when no dialog is open, and hidden/deleted IDs are pruned cleanly when filters or deletions remove cards.
- **Floating bottom bulk action bar:**
  - Centered near the bottom of the viewport in solid ink (`#2b2420` / `bg-foreground text-panel`) whenever 1+ galleries are selected: `✕` clear · `N selected` · **`★ Favorite`** (marks all selected as favorite if any is unfavorited, or unfavorites all if every selected gallery is already favorited) · **`✎ Quick edit`** · **`🗑 Delete`**.
- **Quick edit dialog (`Name` + `Shoot date`):**
  - Lists each selected gallery on a compact Warm Paper row (`cover thumbnail` or `CoverBackdrop` · `Title` `Input` · `Shoot date` `DatePicker` with `clearable`), with `Cancel` and `Save changes` (sending `PATCH /api/v1/galleries/{id}` only for rows whose title or shoot date actually changed).
- **Bulk delete confirmation dialog:**
  - Reuses `ConfirmDialog` showing the count of galleries to permanently delete (including their photos and assets), running `DELETE /api/v1/galleries/{id}` for each selected gallery with busy/non-dismissible state while deletion runs.

## Non-goals

- Adding parallel batch HTTP routes (`POST /galleries/delete` or `PATCH /galleries/batch`): per `REVIEW.md`, extending `GalleryChanges` with `favorite: Option<bool>` on the existing `PATCH /api/v1/galleries/{id}` and reusing `DELETE /api/v1/galleries/{id}` keeps one clean server path (including `WorkspaceDeletion`'s transactional worker cleanup jobs).
- Exposing studio `favorite` status to clients (`SharedGallery` never includes `favorite`).

## Constraints

- **Database schema (`0034_gallery_favorite.sql`):**
  - `ALTER TABLE galleries ADD COLUMN favorite boolean NOT NULL DEFAULT false;`
  - Update `GallerySort::order_by` so `favorite DESC, ...` precedes every sort mode.
  - Include a transaction-scoped migration test and sort-order repository test in `crates/database`.
- **Rust rules (`crates/AGENTS.md` & `REVIEW.md`):**
  - Zero free functions outside `#[cfg(test)]`; `favorite: bool` on `Gallery` (and flattened onto `GalleryOverview` / `GalleryView`, never on `SharedGallery`); `favorite: Option<bool>` with `deserialize_present` on `GalleryChanges`.
  - A `GalleryChanges` patch that modifies *only* `favorite` preserves `updated_at` so starring and later unstarring a gallery drops it back to its true last-edited position rather than artificially jumping its timestamp.
- **Frontend (`web/AGENTS.md`):**
  - Every user-visible string in `web/messages/{en,fr}.json`, zero `vitest-axe` violations, and responsive at `390 px` (compact floating bar and stacked Quick Edit rows on narrow phones).

## Approaches

- **A. Extend `PATCH /api/v1/galleries/{id}` with `favorite: bool` and orchestrate batch saves/deletes from the dashboard over existing endpoints (chosen).** Follows `REVIEW.md` ("Extend an existing route, type or enum... instead of adding a parallel handler"), reuses `WorkspaceDeletion::delete_gallery` cleanup guarantees without duplicating SQL, and keeps partial failure reporting explicit.
- **B. Dedicated batch update and batch delete endpoints.** Adds parallel routes and duplicate validation/cleanup code for small photographer-selected sets; rejected.

## Decisions

- `GallerySort::order_by` prefixes every sort clause with `favorite DESC, `.
- Bulk `Favorite` button behavior: if any selected gallery has `!favorite`, sets `favorite: true` on the unfavorited ones; if all selected galleries already have `favorite: true`, sets `favorite: false` on all selected (label updates dynamically: `Favorite` vs `Unfavorite`).
- Studio page toggle: a subtle icon button (`Star`) in `GalleryStudio`'s identity header lets the photographer star/unstar the open gallery directly as well.
- Update `docs/SPECIFICATIONS.md` (`FR-03`), `docs/releases/MVP.md`, `docs/designs/ui/screens/dashboard.md`, and `docs/designs/ui/screens/gallery.md`.

## Left to the builder

- Responsive layout details of `QuickEditGalleriesDialog.svelte` and the floating bottom bulk bar at `390 px` so labels/icons never overflow horizontally in English or French.
- Handling partial errors cleanly during multi-gallery Quick Edit or Delete (updating succeeded items in state, keeping failed ones selected/open with a clear error toast).
