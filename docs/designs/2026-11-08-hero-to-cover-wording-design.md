# Rename "hero" to "cover" in the English interface

Issue: [#249](https://github.com/alekece/photo-proofing/issues/249). Wording only.

## Where the wording appears

The interface is English only (no message catalogue), so these literals are the whole surface:

- `PhotoTile.svelte`: star button label and tooltip, "Set as hero" / "Remove hero" → "Set as cover" / "Remove cover".
- `PhotosPanel.svelte`: viewer button accessible name and visible label, same two strings.
- `PhotosPanel.svelte`: error alert "Couldn’t update the hero photo…" → "…the cover photo…".
- Tests asserting these strings (`PhotoTile`, `PhotosPanel`, `Placeholder` fixture).

## Out of scope

Identifiers, `hero_id`, the `hero` design token and `--animate-hero-in`, code comments and test names stay unchanged. Specification and design documents still use "hero" as the design term; they are not user-visible and are left for a separate decision.

## Cover signature

The Settings "cover signature" is the studio text drawn on the gallery cover. It is the same cover this wording names (the gallery's main photo, or a default cover when none is set), so "Set as cover" does not suggest a different thing; the signature is only an overlay on it.
