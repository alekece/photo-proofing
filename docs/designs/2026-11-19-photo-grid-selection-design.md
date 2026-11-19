# Photos grid and selection behavior

Issue: [#404](https://github.com/alekece/photo-proofing/issues/404).
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
- Start selection from a top-left circle at the tile corner.
- Show the reusable selection bar after the first selection.
- Keep Download all on Favorites and Final.
- Keep existing upload, processing, retry, viewer and delete behavior.

## Non-goals

- Removing comments, favorites or per-photo viewer actions.
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
- The selection circle is visible on hover/focus and touch, and is positioned at the top-left.
- The circle itself toggles selection mode and the bar appears after the first click.
- Normal ready tiles keep hero, comments and open-viewer behavior outside selection mode.
- In selection mode, clicking a tile toggles it instead of opening the viewer.
- The bar owns close, selected count, Select all and Escape behavior.
- Photos and Final supply Delete; Favorites supplies Download.
- Download all remains the primary action in Favorites and Final even though selection mode can also
   create ZIP subsets.
- Final photos may show only a small retouched/download-status icon; no "Proof", "Final", approval or
   filename text is required over the photo.

## Open questions

1. **Grid columns:** is the default 3 columns or 4 columns on 1280 px and wider?
2. **Layout settings:** should gallery grid/masonry settings remain available in Design, or should the
   Photos/Final review surface always use one compact regular grid?
3. **Selection circle:** should the circle be 20 px, 24 px or 28 px to match existing dashboard
   selection controls?
4. **Final indicator:** is the small retouched icon enough, or should the Final tab label itself remain
   visually distinct with no icon on each tile?
5. **Empty grid:** should empty/processing tiles fill a grid row or retain a dedicated drop zone?
6. **Final deletion:** should Final keep Delete in selection mode, or should Final offer only
   Download and leave delete available only in Photos, matching the existing UI rule?

## Verification

- No top-bar Select control appears on Photos or Final.
- Hover/focus/touch all expose a keyboard-operable selection circle.
- The selection bar does not move or resize the photo grid.
- 390 px shows usable touch targets and no horizontal overflow.
- Context actions can differ across Photos, Final and Favorites.
- Existing hero, delete, viewer, upload and download tests continue to pass.
