# Copy-link control

Issues: #240, #267, #268. Look and motion follow `docs/designs/ui/DESIGN.md` (green `ok`, `prefers-reduced-motion`).

## Intent

The client link field invites editing and Copy gives no visible confirmation. Make the field inactive, put the button beside it, and confirm a successful copy.

## Acceptance

- The field is read-only: text is selectable (useful for manual copying), not editable, with no focus ring on click or tab (the button keeps its own ring).
- The button sits right of the field, labelled **Copy**; after a successful copy it shows a green lucide check and **Copied** for 3 s, then returns to **Copy**.
- Repeated clicks restart the 3 s delay; the change is announced through a `role="status"` region.
- On failure the existing fallback stays: the link is selected and a "copy it manually" message shows; no confirmation.
- The only visible confirmation is the button's green check and **Copied**; the `role="status"` region is screen-reader-only on success (a visible copy duplicated the label below the button) and visible only for the manual-copy failure message.
- The check scales up slightly with a ripple, through `ClickPop.svelte`, the same mechanism the favorite heart uses (same scale keyframes and a ring ripple). Reduced motion makes it instant via the global rule; the check and **Copied** stay visible.

## Controls covered

| Control | Where | Change |
| --- | --- | --- |
| `ShareLink` | gallery settings, client link | uses the shared component |
| `PublishDialog` result | after Share / Deliver | uses the shared component; the dialog now shows the link too |

`BuildInfo` copies a build SHA, not a link, and keeps its own control.

## Approaches

1. Patch each control separately: duplicates timer and a11y logic.
2. A shared `CopyLink.svelte` (read-only field + button + status): one behavior. Chosen.
3. Disabled field: blocks manual selection, rejected.

## Notes

The dialog's old failure text pointed to the page's field; the dialog now has the field itself, so it uses the same manual-copy fallback. Global focus CSS is untouched; the field opts out locally.
