# Maintainer review polish

Issue: [#319](https://github.com/alekece/photo-proofing/issues/319)

## Why

A maintainer walkthrough of the photographer's journey (empty dashboard → new gallery → upload →
share → selection → deliver → settings → archive/delete) found small but visible rough edges, one
delivery bug, and one missing piece of information: the date of the shoot. This design fixes them
in one pass so the app feels finished and consistent.

## Goals

- Every item in #319 is fixed as decided below.
- The photographer can record a shoot date and see it on the gallery page.
- After delivery, Preview shows exactly what the client sees: the finals.
- No success toast anywhere; failures still use error toasts.

## Non-goals

- Moving the other `ui/` components to a component library. That is a separate issue, created
  after this design (see Calendar below).
- Showing the shoot date to the client, on the dashboard card, or anywhere other than the
  photographer's gallery page.
- Changing notification rules beyond "opening a gallery reads it".

## Anti-goals

- A save that can be sent twice because a button only looks inactive.
- A shoot date that shifts by one day across time zones.
- A calendar that is less accessible than the native date input it replaces.

## Decisions

### Dashboard

- **Empty dashboard:** replace the animated film contact sheet in `EmptyWorkspace.svelte` with the
  shared `ui/EmptyState` (message and New gallery action). Delete the unused animation CSS.

### Gallery page header

- **Details card:** the single meta sentence in `GalleryHeader.svelte` becomes a small card in the
  style of the page's existing panels, with two rows: headings on top, values below.
  Columns: **Photos** · **Shot** (or **Created** when no shoot date) · **Last opened** (only once
  shared, as today). The "no photos yet" text (`header_meta_empty`) goes away: an empty count shows
  `0`.
- **Shoot date:** see its own section.

### Shoot date

- New optional gallery field `shoot_date`: PostgreSQL `date` (no time, no zone), API
  `YYYY-MM-DD` or `null`. Existing galleries get `null`.
- Asked in the New gallery dialog, below the title, with the new calendar. Optional: the dialog still
  creates a gallery with a title only. *(Builder call: optional, so creation stays one field away.)*
- Editable on the gallery page next to the other gallery settings (same calendar, clearable).
  *(Builder call: otherwise a wrong date could never be fixed.)*
- Shown only on the photographer's gallery page, where it visually replaces the created date: in
  the details card and in the cover backdrop's edge text (`CoverBackdrop` `edge`), falling back to
  the created date when empty. *(Builder call: one date on the page, never two.)* Not sent to the
  client page.
- Follows the `last_opened_at` path: migration → `database` gallery model, projection, create and
  `GalleryChanges` → `api` create/update DTOs → web `schemas.ts`, `GalleryChanges`,
  `NewGalleryDialog`, gallery settings.
- `docs/SPECIFICATIONS.md` gallery configuration: add the shoot date and change "the New gallery
  dialog asks for a title only" to title and optional shoot date.

### Calendar

- Adopt **shadcn-svelte's Calendar** (copied into `web/src/lib/components/ui/Calendar`, built on
  Bits UI with `@internationalized/date`), styled with the app's tokens. Wrap it in a `DatePicker`
  that opens the calendar in the existing `ui/Popover` from a field-styled trigger.
- Locale follows the active Paraglide language (week start, month and day names). Keyboard and
  screen-reader behaviour comes from Bits UI and is checked in English and French.
- Replace every native `type="date"` input: gallery expiry (`GallerySettings`), restore expired
  (`RestoreExpiredDialog`), and the new shoot date fields. Keep existing min/max rules (expiry
  dates in the past stay disallowed).
- Add an ADR that supersedes the "no new dependency" rule in the frontend design-system design:
  prefer well-known libraries whose components we can fully style, starting with shadcn-svelte.
  Open a separate issue to migrate the remaining `ui/` components.

### Photos tab

- **Upload placeholder:** a soft shimmering skeleton instead of flat grey, from the existing
  placeholder tokens. Status text and progress bar stay.
- **Fade in:** a tile's image fades in (ease-in-out) once loaded, reusing `--animate-photo-in` or a
  sibling token. Reduced motion shows it instantly.
- **Masonry size:** the photographer's masonry tiles look much smaller than grid tiles. Make
  masonry columns match the grid tile width (`GRID_CLASSES` in `PhotoTile.svelte`). Reproduce in
  the browser first; the cause may be the arbitrary `columns-[…]` value.
- **Selection toolbar:** `ui/SelectActions` adopts the look of the app's other toolbars instead of
  its golden bar. Select all and Unselect all become regular `Button`s on the right, with the
  other actions; the tri-state checkbox goes.

### Favourites and Finals tabs

- Favourites caption: keep the time; the heart grows from 12 to 16 px and is red (heart/danger
  token).
- Finals caption: same treatment for the download icon, in green (ok token).

### Notifications

- Opening a gallery page marks its unread notification read. The schema keeps at most one unread
  row per gallery (`notifications_one_unread`), so all kinds are cleared together.
- New workspace-scoped API call that marks a gallery's unread notification read in one update
  (`… WHERE workspace_id = $1 AND gallery_id = $2 AND read_at IS NULL`). The gallery page calls it on
  open (and when the gallery id changes) and clears the badge locally; the existing notification
  event refreshes the bell.
- A notification that arrives while the page stays open is left unread, so the bell still
  announces it. *(Builder call: keeps the rule simple and avoids a read/notify loop.)*

### Share and Deliver results

- Remove the link and Copy button (`CopyLink`) from the result step of `PublishDialog`. The rest of
  the result keeps its current layout and text. `CopyLink` stays where it is used elsewhere.

### Delivered preview (bug)

- `ClientPreview` always loads proofs. When the gallery is delivered it must load the finals and
  build finals thumbnails and previews, laid out as the client's delivered page. Preview stays
  read-only: download controls remain inert as today, so no client download is recorded. No proof
  favourites are passed. If the cover photo is not a final, use the delivered
  page's existing rule for a missing cover.
- Regression test: deliver after removing photos from finals; the preview lists only finals.

### Buttons with a lifecycle

- Applies to Settings Save (Save → Saving… → Saved) and Cancel (Cancel → Reverted), and any other
  button that swaps its label this way.
- Labels must not move when the icon appears: no vertical slide, and the label keeps its position
  (the icon takes reserved space or fades in beside it without pushing the text).
- Keep the ripple on Saved and Reverted.
- During Saving, Saved and Reverted the button keeps its active look (no grey) but ignores
  clicks: `aria-disabled="true"`, and both the button and the page handler ignore activations
  while not idle. It is still `disabled` when there is nothing to save.

### Toasts

- Remove every success toast: Delivered, Archived, Restored, the settings regeneration messages,
  and any other `toasts.success`. The page's own state shows the result; regeneration is shown by
  the photo panel's existing "updating" status.
- Error toasts stay unchanged.

### Reprocessing status

- While a ready photo is reprocessed after a settings change (watermark, quality), its tile keeps
  the old thumbnail and replaces the solid status block with the upload style: the status label
  above the thin animated progress bar at the bottom of the tile. Failures keep their Retry action.

### Client delivered cover

- Remove the "Your photos are ready" line above the title on the client's delivered gallery cover;
  the download actions already say the photos are ready.

### Delete dialog

- The cancel button uses the shared `common_cancel` label (Cancel / Annuler) instead of
  `delete_keep`. Remove the unused message.

## Approaches considered

- **Calendar:** a Bits UI-based shadcn-svelte Calendar (chosen) versus a hand-built calendar.
  Hand-built matches the existing `ui/` pattern with no dependency but would have to re-create
  keyboard navigation, screen-reader support and locale rules. The maintainer prefers well-known
  libraries that can be fully styled.
- **Order:** this issue first, with the calendar as the first library component, versus migrating
  all `ui/` components before. Chosen: this issue first, so the review fixes are not blocked.
- **Delivery:** one PR (chosen) versus three (shoot date, fixes, polish).

## Left to the builder

- Exact card, skeleton, toolbar styling and spacing within the existing design tokens, checked in
  light and dark themes.
- The endpoint path for marking a gallery's notification read.
- Tests for each behaviour change (shoot date round trip and display, notification read on open,
  delivered preview, lifecycle button guarding, no success toasts, calendar locale).
