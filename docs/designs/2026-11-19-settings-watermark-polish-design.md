# Compact settings and configurable watermark

Issue: [#408](https://github.com/alekece/photo-proofing/issues/408).
Builds on:
[settings and watermark polish](2026-11-09-settings-watermark-polish-design.md),
[settings screen](../ui/screens/settings.md),
[proof variants](2026-11-07-proof-derivatives-design.md).

## Intent

Make workspace Settings compact and easier to scan while keeping the live preview. Add configurable
text color and opacity, and avoid expensive proof recomputation after a gallery is published.

## Goals

- Rework Settings as grouped sections with a live preview retained.
- Use compact 1 or 2-column layouts and responsive section navigation.
- Add watermark opacity.
- Default watermark opacity is `50%`.
- Replace fixed text color choices with a custom color picker.
- Use a cream preview surface that works for both black and light logos.
- Preserve original logo colors and transparency.
- Do not recompute proofs for already published/delivered galleries.
- Keep finals, originals and exports unchanged.

## Non-goals

- Removing any existing watermark type, pattern, position or size choice.
- Automatically changing published client proofs.
- Changing cover signatures unless shared settings require it.
- A new watermark library or renderer.

## Approaches

1. **Keep the current long page and append controls.** Rejected: it adds height and weakens grouping.
2. **Tabs plus live preview.** Chosen. Section navigation changes the active control group while the
   preview remains available.
3. **Fixed black/white text color.** Rejected: it is unnecessarily restrictive.
4. **Custom text color plus opacity.** Chosen.
5. **Recompute every proof when watermark settings change.** Rejected for published/delivered
   galleries.

## Decisions

- Settings keeps live preview and compact Save/Cancel lifecycle.
- Desktop uses section navigation plus active controls; mobile uses a horizontal section strip.
- Watermark controls are grouped: enablement, mark, pattern, position, size, text color and opacity.
- Text watermarks use a custom color picker and optional hex text.
- The text-watermark control shows both a native color picker and a hex field.
- Logo/name previews use a cream selected surface; selection itself remains visible through border
  and check indicators rather than selected black fill.
- Logo watermarks keep original logo colors and transparency.
- Effective watermark changes do not regenerate published or delivered proofs.
- Draft and otherwise unpublished proofs may recompute according to the existing worker behavior.
- Draft and Prepublish proofs recompute; published/delivered proofs do not.
- Finals, original images and exports continue to use original files.

## Open questions

1. ~~**Opacity range/default:** 50, 70 or 100?~~ **Resolved:** 0–100%, default `50%`.
2. ~~**Published scope:** Prepublish recompute?~~ **Resolved:** recompute Draft and Prepublish.
3. ~~**Draft/prepublish transition:** scope?~~ **Resolved:** recompute both.
4. ~~**Text color format:** hex, picker or both?~~ **Resolved:** picker plus hex field.
5. **Logo on cream:** should logo uploads receive a neutral drop shadow or remain flat?
6. **Preview layout:** should the live preview remain on the right desktop column and above/below on
   mobile, or stay below every settings section?
7. **Save feedback:** should Saving/Saved return to Save automatically as in existing controls, or
   remain in a sticky action row until the next edit?

## Verification

- Settings sections fit desktop and 390 px screens without duplicated preview markup.
- Custom text color appears in live preview and generated marks.
- Opacity is persisted and applied consistently to repeated and single marks.
- Cream preview is legible with black and white logos.
- Changing settings after publication/delivery causes no worker jobs for proofs.
- Draft/prepublish behavior follows the resolved published-scope rule.
- Existing logo limits and metadata-free PNG behavior remain unchanged.
