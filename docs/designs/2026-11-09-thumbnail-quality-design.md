# Sharper thumbnails and previews

Issue: [#304](https://github.com/alekece/photo-proofing/issues/304). Presets chosen by the photographer are a separate follow-up, [#312](https://github.com/alekece/photo-proofing/issues/312).

## Intent

Gallery tiles look soft. A tile is about 180 CSS pixels wide, two per row on phones. On 2x and 3x screens it needs 360 to 570 pixels, but the thumbnail is 400 pixels on its long edge at JPEG quality 75, so it is stretched and shows compression artefacts.

## Decision

| | Before | After |
| --- | --- | --- |
| Thumbnail long edge | 400 px | 800 px |
| Preview long edge | 1600 px | 2000 px |
| JPEG quality | 75 | 82 |

Maintainer's choice among three options (640 px, 800 px, 1000 px for thumbnails); the preview follows the same quality.

## Cost

Per photo, roughly: thumbnail from 40-60 KB to 150-220 KB, preview from 300-450 KB to 600-900 KB. Grids load tiles lazily, so a gallery only transfers the tiles that are scrolled into view. Storage grows by the same amounts, per proof and per watermarked copy.

## Scope

- Only new uploads get the new sizes. Existing photos keep their current files until they are re-uploaded or their marked copies are regenerated.
- Proofs stay reduced: a 2000 px preview is still too small to print, and nothing here exposes originals.
- Originals, finals and exports are unchanged.
- No setting yet; see #312.
