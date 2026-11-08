# Stable select-mode toolbar

Issues: [#241](https://github.com/alekece/photo-proofing/issues/241),
[#242](https://github.com/alekece/photo-proofing/issues/242),
[#246](https://github.com/alekece/photo-proofing/issues/246),
[#266](https://github.com/alekece/photo-proofing/issues/266). The maintainer pre-approved going straight
to implementation.

## Intent

Select mode must not restyle or move the gallery toolbar, nor pile new buttons on it. It reuses the
existing buttons: **Select** turns into **Select all**, **Upload** into **Delete**, and the only new
control is **Unselect all**. The Favorites **Download** button is selection-aware, with the duplicate
**Download ZIP** button removed.

## Acceptance

- Select and Upload (Photos/Final) or Download (Favorites) keep their size, position and DOM element in and out of select mode.
- In select mode **Select** reads **Select all** and **Upload** reads **Delete** (disabled at 0); on exit both return to normal. No other buttons appear.
- **Unselect all** is the one added control: it clears the selection and stays in select mode.
- Escape leaves select mode.
- Favorites: **Download all** by default and with no selection; **Download X** with X selected, downloading only those; no download button when there are no favorites. No **Download ZIP** button. Favorites has no Delete, so only **Select** swaps there.
- Same behaviour on Photos and Favorites.

## Decision

- Select mode no longer has a toggle, so **Select** is not `aria-pressed`; its label is the state.
- Left group (`SelectActions`): "N selected" and **Unselect all**. It takes the spare space (`mr-auto`),
  so nothing to its right moves.
- Conflict resolved: with **Select** now "Select all", the issue leaves no button to exit select mode
  besides Escape, which touch users lack. **Unselect all** therefore reads **Cancel** at 0 selected and
  leaves select mode; otherwise it clears the selection.
- After a confirmed deletion, focus returns to the button that opened the dialog (now **Upload** again).
  Leaving by Cancel or Escape focuses **Select**.
- Favorites' **Download** is always on the right and only its label changes. The "ready ZIP" link is
  removed; a ZIP is still downloaded automatically once the requested export is ready.
