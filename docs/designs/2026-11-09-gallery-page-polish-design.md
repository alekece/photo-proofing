# Gallery page polish: selection bar, drop zone, dialogs

Issue: [#294](https://github.com/alekece/photo-proofing/issues/294). The sticky bar was shown as a mockup and validated by the maintainer. Decisions marked *my call* are the author's and open to change in review.

## Intent

Five rough edges on the photographer's gallery page: a redundant "Saved" toast, a selection toolbar where "Unselect all" sits far from "Select all", an upload button duplicated in the drop zone, a dismiss label ("Pas encore") that reads oddly, and a success dialog that looks unfinished.

## Goals

- Saving a gallery setting shows no "Saved" toast. Failures still show.
- Selecting photos (Photos, Finals and Favorites tabs) uses one sticky contextual bar, shown only in select mode, replacing the separate Select all / Unselect all buttons:
  - icon-only ✕ on the left, labelled "Exit selection", clears the selection and leaves select mode; Esc does the same;
  - live "N selected" count;
  - a tri-state "Select all" checkbox (unchecked, indeterminate, checked); clicking it when everything is selected unselects all;
  - only the actions that make sense for a selection on the right: Delete on Photos and Finals, Download on Favorites; both disabled at 0 selected. Upload is hidden while selecting;
  - accent-tinted background; on small screens it collapses to `[✕] N selected` with icon-only actions.
- The drop zone on Photos and Finals no longer has an upload button (the toolbar one stays; the zone keeps drag and drop).
- Confirmation dialogs dismiss with "Cancel" / "Annuler".
- In the "Shared" success dialog the green check lines up with its text, "Done" is a real button, and a ✕ sits in the top-right corner.

## Non-goals

- The Save button lifecycle on Settings (#297).
- New selection actions or keyboard shortcuts beyond Esc.

## Approaches

1. **Move "Unselect all" next to "Select all" in the current toolbar (rejected).** Fixes the distance but keeps two text buttons for one idea.
2. **One sticky contextual bar (chosen, validated by the maintainer).** The bar is the select-mode state: exit on the left, count, one tri-state control, actions on the right. It stays visible while scrolling a long gallery.

## Decisions

- **Actions:** *My call:* Delete (destructive style) on Photos and Finals, Download on Favorites; selecting for upload makes no sense, so the toolbar's Upload button is hidden in select mode instead of turning into Delete.
- **Component:** `ui/SelectActions` becomes the bar (count, tri-state checkbox using the existing `ui/Checkbox`, ✕ button, an actions snippet). `PhotosPanel` and `FavoritesPanel` render it as the sticky first row of the grid in select mode and drop their own "Select all" button; "Select" in normal mode stays.
- **Esc:** handled by the panel while selecting; it must not fire while a dialog or the viewer is open. *My call.*
- **Messages:** add `selection_exit` and `selection_select_all`; the existing `action_unselect_all` is removed if unused. The count is localized (it is a hard-coded English "selected" today).
- **Drop zone:** remove the button in the empty drop zone and its hidden file input handler only if nothing else uses it; the toolbar upload button keeps opening the picker. *My call:* an empty gallery keeps the toolbar button as the click path, so the zone text stays "Drop photos here" without a "browse" hint.
- **ConfirmDialog:** the default `cancelLabel` becomes `m.common_cancel()`; the `dialog_not_yet` message is removed from both languages.
- **Saved toast:** remove `toasts.saved()` on the gallery page; the existing "Not saved" retry line and error toast stay.
- **Shared dialog:** align the check icon with the heading line, and render the `result` action as the primary `Button` instead of ghost text. Both a real "Done" button and a corner ✕ (labelled "Close"), as the maintainer asked.

## Testing

- `SelectActions` tests: count, ✕ exits, tri-state states, select-all toggling, Esc.
- Panel tests: bar only in select mode, Delete disabled at 0, no drop-zone button.
- `ConfirmDialog` test: default dismiss label.
- Page test: no toast on a successful save, error toast on failure.

## Left to the builder

Colours, spacing and the small-screen layout, within `docs/designs/ui/DESIGN.md`.
