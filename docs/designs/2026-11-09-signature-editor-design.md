# One preview for the watermark and the cover signature

Issue: [#293](https://github.com/alekece/photo-proofing/issues/293). Builds on [#314](https://github.com/alekece/photo-proofing/pull/314) (static sample tiles, French labels). Choices marked *my call* are the author's and open to change in review.

## Intent

The watermark and the cover signature both say how the studio appears (name, logo, both). Settings shows them as two unrelated editors with different previews. They should look and work the same, and the photographer should be able to size the cover signature, which is fixed today.

## Goals

- The page keeps its order: watermark section, then cover signature section.
- Each section has **one big preview** with its toolbar **below** it. Both previews use **the same static sample image** (no photo is loaded, so every workspace sees the same one, as with the tiles today).
- The watermark preview is framed as a proof; the cover signature preview is framed as a client cover. No toggle: each section shows its own frame.
- The cover signature toolbar matches the watermark one: name, logo, both, none, plus a **size slider**. It has no position control.
- The cover signature size is saved with the workspace settings and applied to the client cover.
- Watermark behaviour is unchanged.

## Out of scope

- Moving the signature on the cover, or a per-gallery size.
- Loading the photographer's own photo into the previews.

## Approach

- **Size value.** `workspace_settings` gets `signature_size` (smallint, 0 to 4, default 2), the same five steps and validation as the watermark `size` (`SizeStep`). Step 2 reproduces today's look exactly, so existing studios see no change. *My call:* the same scale as the watermark, so both sliders feel alike.
- **Client cover.** The public gallery response carries `signature_size` next to `signature`. `StudioSignature` scales its font and logo from the step (a fixed table of multipliers, with step 2 equal to 1) instead of the hard-coded sizes at its call sites. Layout stays as is.
- **Regeneration.** None: the signature is drawn by the browser, not baked into proofs. Saving a new size, like saving the signature choice, does not requeue anything.
- **Settings screen.** A shared toolbar piece holds the option buttons and the slider; the watermark editor passes its extras (pattern, position) and the signature editor passes none. The sample is one static image shared by both previews, with the cover frame adding the title block.
- **API.** `signature_size` joins `GET` and `PUT /api/v1/settings` and the shared gallery response, validated like `size`.

## Documentation

`docs/SPECIFICATIONS.md` gains a sentence under optional watermarking: the studio signature on client covers is saved once per workspace, with the same name/logo/both/none choice and a size. README settings text is updated. This is a product change and needs the maintainer's approval, which is given for this work.

## Testing

- Database: the size round-trips and defaults to 2; saving it requeues nothing.
- API: settings and shared gallery responses carry it; out-of-range values are refused.
- Web: the slider saves the size, the cover signature scales with it, the watermark editor still behaves as before, both previews share the sample, English and French labels exist.
