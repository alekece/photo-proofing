# Gallery page polish: implementation plan

Design: `docs/designs/2026-11-09-gallery-page-polish-design.md`. Issue: #294.

1. **Quick wins.** Drop `toasts.saved()` from the gallery page and update its test; `ConfirmDialog` defaults to `common_cancel` and `dialog_not_yet` goes; `PublishDialog` result gets an aligned check, a primary "Done" button and a corner close button.
2. **Selection bar.** Rebuild `ui/SelectActions` (✕, localized count, tri-state `ui/Checkbox`, actions snippet, accent tint, sticky, icon-only actions on small screens). Add `selection_exit` and `selection_select_all`; remove `action_unselect_all` if unused.
3. **Panels.** `PhotosPanel` and `FavoritesPanel` render the bar as a sticky row in select mode with Delete / Download, hide Select, Upload and Select all while selecting, and ignore Esc under a dialog or the viewer. Remove the drop-zone Upload button.
4. **Tests and checks.** Update `SelectActions`, panel, `ConfirmDialog`, `PublishDialog` and page tests; run `pnpm --dir web verify`.
