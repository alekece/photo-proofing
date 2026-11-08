# Wider gallery side navigation

Issue: [#269](https://github.com/alekece/photo-proofing/issues/269)

## Problem

The gallery side nav is 18.75rem (300 px, as in `docs/designs/ui/prototype.html`) from `md` up. The
Client link card (Copy link control, see `ShareLink`) and the Open until / Included photos rows feel cramped.

## Decision

From `md` (768 px) the side nav column is **21rem (336 px)**, +36 px. Below `md` it stays the 3.5rem icon rail.

- Content: cards have 1.25rem side padding, so 296 px of card remains; the link, Copy link button and
  the fixed-width date (8.75rem) and number (4.5rem) inputs fit with room to spare.
- Main area: at 768 px the tabs keep 432 px; at 1024 px, 688 px. The column is not made wider (e.g. 24rem)
  because that leaves too little for the photo grid at the `md` breakpoint.
- Phone popovers (`min(18.75rem, 100vw - 5rem)`) are unchanged: they float over the content and are
  limited by the viewport, not by the column.

The UI design docs keep the prototype's 300 px reference; this design supersedes it for the side nav width.
