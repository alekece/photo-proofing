# Polish the Settings previews

Tracked by [#464](https://github.com/alekece/photo-proofing/issues/464).

Keep Save and Cancel in the shared sticky header, on the page background without a
separate border. Switching tabs must not shift the actions when scrolling changes.
On desktop, keep the Settings section menu in place while the content scrolls.

Keep the controls above each preview. Distribute the groups across the available
width and let them wrap on narrow screens. In Watermark, keep Mark, Pattern and Color
separate, and group Size with Opacity without adding a heading. Use short Mark and Size
labels on both tabs. Center mark button contents, use the same logo size in both, and
indicate selection without a checkmark. The single-watermark placement
buttons have smaller outlines without shrinking the icons or hit areas. Remove the
per-gallery activation note without changing activation.

Repeat the mark across the entire preview, including at the smallest size. Match the
worker's diagonal angle and stagger, sizing the repeated area from the preview and the
stamp instead of using a fixed grid. Do not change generated photos or the cover layout.

Use the same swatch and editable hex field on both tabs. Clearing the signature hex field
restores automatic color without a separate reset button. Do not add signature opacity.
Keep invalid text out of the preview and saved settings.
Cancel rolls back both drafts. Verify on desktop with synthetic data; no mobile redesign.
