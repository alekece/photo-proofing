# Settings and watermark polish

Issue: [#297](https://github.com/alekece/photo-proofing/issues/297). The logo limit change (any dimensions, about 100 MP safety cap, about 20 MiB configurable byte cap) is already approved by the maintainer. Decisions marked *my call* are the author's and open to change in review.

## Intent

Fix a handful of rough edges on the studio Settings page and in the watermark itself: a switch knob that sits too far right, two French labels, a preview that borrows the owner's cover photos, proofs whose repeated mark is not tilted like the preview, a needlessly strict logo upload, and a Save button that relies on a toast.

## Goals

- The watermark switch knob has the same margin from the track edge when on as when off.
- French: "Activer les filigranes" (switch) and "Paramètres du filigrane" (studio heading).
- The preview uses fixed sample tiles drawn in CSS/SVG, identical for every workspace, and the settings loader stops listing galleries.
- Rendered repeated watermarks follow the preview's pattern: rows tilted about -24°, every other row shifted by half a step.
- Logo upload takes any dimensions; it is still stored as a metadata-free PNG of at most 512 px. The 4 MP cap becomes a ~100 MP decompression-bomb cap, and the 1 MiB cap becomes an operator setting (default 20 MiB).
- Save shows its own lifecycle (Save, "Saving…", "Saved" with a green check and ripple, Save); no "Saved" toast. Cancel plays the same ripple, then returns to disabled, and the revert is announced.
- Motion respects reduced-motion.

## Non-goals

- The cover signature and the watermark editor changes of #293, which builds on this and lands after it.
- Pixel-exact equality between the CSS preview and the rendered proof (see below).
- Any new watermark option, or changes to the single-mark placement.

## Constraints

- All text is a Paraglide message in `en.json` and `fr.json`.
- Proofs are rendered by `image-worker` with libvips; the web preview is only an illustration.
- `docs/SPECIFICATIONS.md` FR-07 changes: the logo sentence ("at most 1 MiB and 4 megapixels") becomes any dimensions, a decompression-bomb cap and an operator-configurable byte cap. The README gets the new setting.

## Approaches

### Diagonal repeated watermark

Today `repeated()` tiles axis-aligned staggered rows; the preview rotates the whole grid by -24°.

1. **Rotate the whole overlay (considered).** Tile as today on a canvas big enough to cover the rotated photo (about 1.3× the area), rotate once with `ops::rotate`, crop to the photo. Simple and keeps the existing tiling, but resamples all the text and logo edges once, softening them, and builds and rotates a large RGBA canvas for every proof.
2. **Rotate the mark once, place copies on a rotated lattice (chosen).** Rotate the small mark (shadow included) once with a high-quality interpolator, then compute each copy's position from the same staggered lattice rotated by -24° about the photo's centre, and composite them with the multi-overlay `composite` (x/y arrays, clipping at the edges). The mark is sharp, the cost is one small rotation plus about 50-200 cheap composites, and no big intermediate canvas is built.
3. **Rotate only the mark and keep horizontal rows (rejected).** Looks tilted per mark but the rows stay straight, which does not match the preview.

### Preview sample tiles

1. **Keep the owner's covers, fall back to plain tiles (rejected).** Needs the gallery list, differs per workspace, and leaves empty workspaces with a poor preview.
2. **Static CSS gradients inline (chosen).** Six fixed tiles of plain CSS gradients and simple SVG shapes (sky, sand, forest, dusk, snow, night), mixing light and dark areas, so a white mark with a shadow is visibly checked on both. No image files and no loader data.

### Save button lifecycle

1. **Keep it inline in the page (rejected).** The page already carries the state, timers, the live region and the cancel animation; the same lifecycle will be wanted elsewhere.
2. **A reusable `SaveButton` (chosen).** The page owns the status (`idle`, `saving`, `saved`) and the short "saved" timer; the component owns the look, the check, the ripple and the announcement.

## Decisions

- **Switch:** `ui/Switch` on-position offset goes from `1.375rem` to `1.1875rem`; check the Switch test for the old value.
- **Messages:** `settings_watermark` (fr) becomes "Activer les filigranes", `studio_watermark` (fr) becomes "Paramètres du filigrane". Add `studio_saving` and `studio_saved` (both languages); `studio_reverted` stays for the announcement only, and `toast_saved` is removed if nothing else uses it.
- **Loader:** `settings/+page.server.ts` drops `listGalleries`, `previewUrl`, `samples` and `cover`; it returns `idleTimeoutMs` and `settings`. `WatermarkEditor` loses its `samples` and `cover` inputs and renders the fixed tiles; the page test no longer needs gallery fixtures. *My call:* the cover-signature preview, which today uses `cover`, shows a fixed sample tile with a placeholder title (a message) so it too is identical for every workspace; #293 reworks it.
- **Save button:** `SaveButton.svelte` props are `status`, `disabled` and `onclick`. Idle shows "Save"; saving shows "Saving…", disabled; saved shows a green check and "Saved" with the ripple, for about 1.6 s, then the page sets `idle`. Failures keep surfacing through the existing error toast, and status returns to `idle`. Announcements go through one visually hidden `aria-live="polite"` region, not the label swap. *My call:* the ripple is a shared piece extracted from `ClickPop` (the `click-ripple` keyframes already exist) rather than a second animation.
- **Cancel:** after a revert the button plays the same ripple (no sliding label, no `animate-revert-spin`, which is deleted from `app.css`), then returns to disabled; the live region announces "Reverted" (`studio_reverted`).
- **Reduced motion:** the existing `prefers-reduced-motion` block in `app.css` already makes animations instant; the ripple adds nothing that needs more, but the state must still change (check and label appear, ripple does not show).
- **Rotation in the worker:** `WatermarkPainter` gets `const REPEAT_ANGLE: f64 = -24.0` (degrees, same as the preview) beside `REPEAT_GAP`. `repeated()` is replaced by a function returning copy positions on the rotated staggered lattice (step from the rotated mark's un-rotated size and `REPEAT_GAP`, rows half-shifted), starting beyond the photo's edges so corners are covered, and `paint` composites all copies in one call. Rotate with bicubic interpolation on premultiplied alpha, so edges do not get dark fringes. The rotated mark keeps its transparent padding so shadows are not clipped. The preview's gaps are in `em`, so the lattice matches it in angle and stagger, not to the pixel.
- **Logo limits:** a new `UploadConfig` field `logo_max_size` (`--logo-upload-max-size`, `PP_LOGO_UPLOAD_MAX_SIZE`, default 20 MiB, `NonZeroU64`, next to `PP_PHOTO_UPLOAD_MAX_SIZE`). `LogoNormaliser` takes the cap as a parameter instead of `MAX_BYTES`, and `routes/settings.rs` derives its JSON body limit from it (base64 adds a third) instead of the constant. The pixel cap stays a constant, `DEFAULT_MAX_PIXELS` (100 MP, the same as photos), not an operator setting. *My call:* one fewer knob; the worker's cap is the same number.
- **Shrink on load:** read the header (libvips is lazy), reject above the pixel cap, then shrink while loading with `thumbnail_buffer` limited to 512 px (JPEG shrinks in the decoder; PNG is read sequentially), so a 100 MP logo never sits fully decoded in memory. Orientation, sRGB, alpha and "no metadata" behave as today. Cases that failed before on size now succeed or fail as `logo_too_large` only past the new caps.
- **Docs:** update FR-07 in `docs/SPECIFICATIONS.md` and the README upload bounds list with `PP_LOGO_UPLOAD_MAX_SIZE`.

## Testing

- `watermark.rs`: painting a repeated mark on a black photo lights pixels on many distinct rows, and within one lattice row the marks' vertical positions differ (so they are not all on one horizontal line, which the old axis-aligned tiling would fail). Existing repeat and single-placement tests keep passing.
- `logo.rs`: a large image above 4 MP (and under the cap) is accepted and stored at most 512 px, metadata-free; an image over the pixel cap and an upload over the byte cap are rejected; the cap comes from the parameter. `config.rs` default test covers 20 MiB.
- Web: Switch knob offset; French labels; `SaveButton` states, ripple, live region and reduced motion; settings page shows no "Saved" toast, Cancel ripples then disables and announces; the loader no longer calls the galleries API; the editor renders the static tiles with no images.

## Left to the builder

The tile artwork and exact colours, the check and ripple timings, the lattice step maths, and the exact `thumbnail_buffer` options, within `docs/designs/ui/DESIGN.md`.
