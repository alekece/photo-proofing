# Prompt to update the expiry when restoring an expired gallery

Issue: #175. Builds on `2026-11-06-gallery-lifecycle-design.md` (#73), which defers this prompt. Product rules: FR-13 and AC-06 in `docs/SPECIFICATIONS.md`.

## Intent

Restoring an archived gallery does not bypass its expiry. If the gallery has also expired, clients still cannot open it and nothing says why. The photographer should learn this at the moment of restoring and be able to fix it right there.

## Goals

- After a restore, if the gallery is still expired, tell the photographer clients cannot see it yet, and offer to set a new expiry date or clear it, from the same prompt.
- Dismissing leaves the gallery restored but expired.

## Non-goals

- Changing restore: it still succeeds, never touches delivery or expiry, and returns the gallery as today.
- A new endpoint or a restore option carrying an expiry. The date is saved through the existing gallery update (`PATCH`, `expires_at`).
- Prompting on archive, on opening an expired gallery, or on the dashboard.
- Removing the client event stream (#174); this work does not use it.

## Constraints

- Layout, wording and behaviour follow the approved page design. The prototype has no restore prompt (its restore only shows a "Restored" toast), so the prompt is built from the existing confirm dialog pattern ("Confirm, never block", `docs/designs/ui/DESIGN.md`). Anything the prototype lacks is marked *my call* below.
- The expiry write must reuse the gallery page's `save` path, so the Saved/error toasts, 401 handling and `states` refresh behave as for the side nav's "Open until" field.
- The web app has no i18n layer yet (`2026-11-06-ui-languages-design.md`, #187). Strings are written in English like the current dialogs and listed below so the language work can translate them; the prototype's `restore`/`restored` keys already have French ("Restaurer", "Restaurée").

## How the frontend knows

`POST /galleries/{id}/restore` returns the gallery including `states`. Expiry is derived on the server from `expires_at`, so the restored gallery lists `expired` exactly when `expires_at` is in the past. After a successful restore the page first flushes pending debounced edits, then merges `archived_at`, `states` and `updated_at` from the response (not a wholesale replace, so unsaved fields survive), then prompts if `states` includes `expired`. No extra request, and no client-side clock comparison.

## Approaches

1. **Dialog after restore (chosen).** Restore runs immediately (no confirmation: it is reversible and non-destructive). If the response is still expired, a dialog opens over the restored page. It is built on the lower-level `Dialog`, not `ConfirmDialog` (one confirm action, no form body), and reuses the same look.
2. **Toast with an action.** Fits "Restored", but a date picker cannot live in a toast, so it would link to the side nav's "Open until" field. Easy to miss and not "from the same prompt". Rejected.
3. **Ask before restoring.** Would need the expiry before restore and either a combined endpoint or two calls where restore could fail half-way. Contradicts "restore unchanged". Rejected.

## Prompt design

- **Trigger.** Only after a restore whose response is expired. Not shown when the gallery has no expiry or a future one.
- **Content.** Title "Clients can't see this gallery yet" (my call). Body: "It was restored, but its link expired on {date}. Set a new date or remove the expiry so clients can open it." The date is formatted like the Publish dialog's short date (e.g. "7 Nov"); `format.ts` has no expiry formatter, so share or add one helper.
- **Fields.** One date input labelled "Open until", empty by default (my call: no guessed date), with the same local end-of-day conversion as the side nav. The minimum is today so a past date is refused in the browser; a typed or pasted past date, which bypasses `min`, is not saved and shows an inline error "Pick a date from today on." (my call).
- **Actions.**
  - *Set date* (primary): disabled until a valid date is entered; saves `{ expires_at }`.
  - *No expiry*: saves `{ expires_at: null }`.
  - *Keep expired* (dismiss, also Escape and backdrop): closes with no request. The gallery stays restored and expired; its chip and the side nav's "Open until" field keep saying so.
- **Result.** The dialog calls the page's `save`, which returns a boolean and never throws (it also handles 401). On `true` the dialog closes; `states` refresh drops Expired. The restore itself shows "Restored" (as the prototype) and the date save the usual "Saved" toast; both are fine together. On `false` the dialog stays open, `save`'s error toast explains, and the photographer can retry or dismiss. A failed or 401 restore shows an error toast and no prompt.
- **Accessibility.** Standard dialog: focus moves to the date input, is trapped and returns to the Restore button on close; the date input has a real label; the inline error is tied to the input (`aria-describedby`) and announced; everything works by keyboard.
- **Motion and mobile.** As other dialogs (scale and fade, centred, fits the width); instant under reduced motion.

## Strings (English, for #187 to translate)

Clients can't see this gallery yet · It was restored, but its link expired on {date}. Set a new date or remove the expiry so clients can open it. · Open until · Set date · No expiry · Keep expired · Pick a date from today on.

## Decisions

- Restore needs no confirmation; the prompt only follows it. *My call.*
- Dismissal is not remembered: reopening the gallery does not re-prompt. The expired chip and the field already show the state. *My call.*
- Restoring from the dashboard is out of scope: the restore action lives on the gallery page (header and side nav), and both entry points share the same handler, so both prompt.
- The prototype is not changed; the prompt is a small addition to the approved dialogs. If the owner wants it drawn, that is a follow-up to the page design.

## Verification

Web tests (vitest, mirroring `GalleryDialogs.test.ts` and `GallerySettings.test.ts`): no prompt when the restored gallery is not expired (no expiry, future expiry); prompt when expired; Set date sends the end-of-day value through the update and closes; No expiry sends `null`; Keep expired and Escape send nothing and leave the gallery restored and expired; past date blocked; failed save keeps the dialog open; focus return. Existing API tests already cover restore keeping `expires_at` and the expired state; add one only if the builder finds that gap. Run `pnpm --dir web verify`.

## Left to the builder

Wiring restore from the stub in the gallery page (`restore()` currently shows a "later release" toast). The prompt is only reachable once archive works in the web app, so it depends on #73's web work landing first; the builder tests it with a seeded archived gallery until then. Component name and structure, and exact date-input details, are the builder's.
