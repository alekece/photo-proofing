# Stable select-mode toolbar

Issues: [#241](https://github.com/alekece/photo-proofing/issues/241),
[#242](https://github.com/alekece/photo-proofing/issues/242),
[#246](https://github.com/alekece/photo-proofing/issues/246). The maintainer pre-approved going straight
to implementation.

## Intent

Toggling select mode must not restyle or move the gallery toolbar. Select mode adds bulk actions
(including a new **Unselect all**), and the Favorites **Download** button becomes selection-aware, with
the duplicate **Download ZIP** button removed.

## Acceptance

- Select, Upload (Photos/Final) and Download (Favorites) keep their size and position in and out of select mode.
- **Select** is a toggle with `aria-pressed`, filled when active; pressing it again, or Escape, leaves select mode.
- **Unselect all** sits beside **Select all**, clears the selection, stays in select mode, and is disabled at 0.
- Favorites: **Download all** by default and with no selection; **Download X** with X selected, downloading only those; no download button when there are no favorites. No **Download ZIP** button.
- Same behaviour on Photos and Favorites.

## Approaches

1. Keep the replacing "pill" and restyle it to match. Still swaps controls and shifts the bar. Rejected.
2. **Fixed right-hand controls plus a left selection group (chosen).** The existing controls stay
   mounted and unchanged; select mode only adds a group in the free space on the left.

## Decision

- Left group (`SelectActions`, replaces `SelectPill`): "N selected", **Select all**, **Unselect all**,
  and the tab's own action (**Delete** on Photos/Final) as a slot. It takes the spare space (`mr-auto`), so
  nothing to its right moves.
- Favorites' **Download** is always on the right and only its label changes. The "ready ZIP" link is
  removed; a ZIP is still downloaded automatically once the requested export is ready.
- The ✕ cancel button is dropped; the pressed **Select** and Escape replace it.
