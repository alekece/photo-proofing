# Photos grid and selection behavior

Issues: [#404](https://github.com/alekece/photo-proofing/issues/404), [#447](https://github.com/alekece/photo-proofing/issues/447).
Existing design:
[larger staggered photos](2026-11-08-larger-staggered-photos-design.md),
[stable select-mode toolbar](2026-11-08-select-mode-toolbar-design.md),
and [gallery screen](../ui/screens/gallery.md).

## Intent

Make the photographer Photos view image-first and selection discoverable without adding a permanent
toolbar Select button. Finals remain visually separate through their tab and an unobtrusive
retouched indicator, not overlays or filenames on every photo.

## Goals

- Rename All Photos to Photos.
- Remove proof/approval-style overlays and photo filename labels from normal tiles.
- Use a consistent compact desktop grid with no blank vertical gaps.
- Use adaptive 3 or 4 columns based on the available container width.
- Start selection from a top-left circle at the tile corner.
- Show the reusable selection bar after the first selection.
- Keep Download all on Favorites, independent of the working selection.
- Keep upload, processing, retry and viewer navigation. Delete photos only through the selection bar and confirmation.

## Non-goals

- Removing comments, favorite state or cover selection.
- Changing original or final download semantics.
- Changing client-facing photo grids.

## Approaches

1. **Keep the top Select button.** Rejected: it is redundant and duplicates selection entry.
2. **Keep masonry and only remove labels/overlays.** Rejected: masonry can retain unused gaps and does
   not meet the audit's compact render preference.
3. **Use a regular compact grid, preserve optional grid density settings if product design approves
   them.** Chosen as the default direction for the photographer review surface.
4. **One shared selection bar with context actions supplied as snippets.** Chosen.

## Decisions

- The Photos tab removes its top-bar Select control.
- The selection circle is `26 px`, matching the existing dashboard card selection control.
- The selection circle is visible on hover/focus and touch, and is positioned at the top-left.
- The circle itself toggles selection mode and the bar appears after the first click.
- Normal ready tiles keep hero, comments and open-viewer behavior outside selection mode.
- In selection mode, clicking a tile toggles it instead of opening the viewer.
- The bar owns close, selected count, Select all and Escape behavior.
- Photos and Final supply Delete in selection mode; Favorites supplies Download.
- Download all always exports every favorite; Download selected exports exactly the checked favorites. No tile or viewer offers individual deletion or download. Finals retain only their passive client download-status indicator.
- In the main Photos page, mixed proof/final sets may use a small retouched icon on Final items.
   The dedicated Final tab in Gallery view shows no per-photo indicator; no "Proof", "Final",
   approval or filename text is required over a photo.

## Open questions

1. ~~**Grid columns:** 3, 4 or adaptive?~~ **Resolved:** adaptive 3 or 4 by container width.
2. ~~**Layout settings:** keep masonry/grid setting?~~ **Resolved:** keep existing layout settings.
3. ~~**Selection circle:** size?~~ **Resolved:** `26 px`.
4. ~~**Final indicator:** per-photo text/icon?~~ **Resolved:** small retouched icon only in the main
   Photos page; no per-photo indicator in the Gallery Final tab.
5. ~~**Empty grid:** where do uploading tiles go?~~ **Resolved:** uploading and failed tiles stay in
   the photo grid; a dedicated drop zone appears only when the list is empty.
6. ~~**Final deletion:** delete in selection mode?~~ **Resolved:** keep Delete in Final.

## Verification

- No top-bar Select control appears on Photos or Final.
- Hover/focus/touch all expose a keyboard-operable selection circle.
- The selection bar does not move or resize the photo grid.
- 390 px shows usable touch targets and no horizontal overflow.
- Context actions can differ across Photos, Final and Favorites.
- Existing hero, delete, viewer, upload and download tests continue to pass.
