# Debounced "Saved" toast

Issue: [#239](https://github.com/alekece/photo-proofing/issues/239)

## Problem

The settings page shows an inline "Saved" state on its Save button (and a hidden status line). It is easy to miss and
repeats for every save. Gallery edits show no success confirmation at all.

## Surfaces that save

- Workspace settings page (`routes/settings`): explicit Save button, one request.
- Gallery page (`routes/dashboard/[id]`): title, layout, included photos, expiry and other settings, each auto-saved
  from one `save()` function.
- Hero photo toggle in `PhotosPanel`: separate `updateGallery` call.

Failed saves already report through error toasts (or the title's inline "Couldn't save" with Retry) and are unchanged.

## Approaches

1. Debounce in every page. Rejected: three copies of the same timer logic.
2. A debounced `toasts.saved()` on the existing toast store, with the quiet period as one exported constant
   (`SAVED_QUIET_MS`, 3 s). Pages call it after a successful save. **Chosen.**
3. Show a toast per save and merge duplicates. Rejected: it would still appear immediately and repeatedly.

## Behaviour

- Each call to `toasts.saved()` restarts the timer; after 3 s without another call one "Saved" success toast appears.
- The Save button no longer swaps to a "Saved" face; it returns to its disabled "Save changes" state. The Cancel
  "Reverted" feedback is untouched.
- A failed save does not call `saved()`; a pending toast from an earlier success still fires.
