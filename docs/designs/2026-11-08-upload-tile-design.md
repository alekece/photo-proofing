# Upload tile status design

Issue: [#245](https://github.com/alekece/photo-proofing/issues/245). Reference: `docs/designs/ui/screens/gallery.md` (Upload).

## Intent

While a photo uploads, its status label must sit directly above the tile's progress bar and use the
same type as the other tile status labels.

## Prior work

#224 (criterion 1) kept the label and motion inside the tile, but the label stayed vertically
centred, far from the bar, and used a lighter weight than the strip label shown on clean-ready tiles.
This design only adjusts placement and type; states, events and motion are unchanged.

## Acceptance

- Waiting, Uploading and Processing show the label immediately above the bottom progress bar; Failed
  has no bar and keeps its centred red label with reason and Retry.
- All tile status labels use one style: system font, `text-xs`, semibold.
- Placement is tile-relative, so it holds in the grid and staggered layouts, empty or populated.

## Approaches

1. Move the label into a bottom-anchored block above the bar (chosen): small, keeps the existing markup.
2. Rework the tile into separate overlay components: larger, conflicts with in-flight tile work.

## Chosen

The placeholder overlay becomes a column: the file name is centred in the free space, the label sits
at the bottom with room for the bar. A shared `STATUS_TEXT` class styles every label.
