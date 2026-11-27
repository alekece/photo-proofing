# Polish the gallery Studio

Tracked by [#465](https://github.com/alekece/photo-proofing/issues/465).

## Intent

Make one gallery easier to scan and manage without changing the client journey or lifecycle
rules. The Studio should use the same visual language as Galleries, Photos, Contacts and
Settings, while keeping every expensive or global action explicit.

## Goals

- The gallery side panel keeps a quiet "‹ Galeries" link back to the dashboard.
- Reuse the dashboard's workspace `StatusChip` treatment and favorite-pin component in the
  Studio cover identity. The status has no dot. Dates move below the editable title and show
  only when present: "Shot on / Séance du [date]", expiry, then last opened. Leave extra breathing
  room before the Collection navigation group.
- Gallery cards summarize all available counts in one comma-separated line: photos, selected,
  retouched and downloaded. Retouched and downloaded counts are shown whenever finals exist,
  including before delivery.
- Use five Journey steps: **Upload**, **Share**, **Collect favorites**, **Import finished photos**,
  **Deliver photos**. The bar stays a progress/navigation surface; contextual actions live in
  the selected panel instead of on the right of the Journey.
- Keep the current terracotta marker for the active Journey step. `Collect favorites` opens
  Favorites and stays active after sharing until an explicit, successful **Download all**.
  Downloading one or more selected favorites does not complete it.
- After the full favorites download, keep the marker on **Import finished photos** until the
  first finished photo exists, then move it to **Deliver photos** until delivery is confirmed.
  The click target for Upload opens Photos, Share opens the share action, Collect opens
  Favorites, Import opens Final, and Deliver opens the existing delivery action.
- Keep Photos unchanged. In Favorites and Final, dates use a shared readable glass caption over
  full-image tiles and a normal text caption below Print tiles. Favorite hearts remain red and
  Activity uses that same red for favorite events.
- Give full-image Favorites and Final captions equal heights and equal 24 px icon footprints.
- Remove the permanent ZIP status block from Favorites. Opening the tab never checks an old
  export. An explicit Download request creates or reuses the matching ZIP; subsequent status
  failures become dismissible feedback and a second click retries the request.
- Populate Final with several synthetic finished photos, including downloaded timestamps.
- Shorten Design copy to "Change", "Remove", and "Style". Change and Remove occupy equal halves
  of one full-width row and both have an icon. Keep density/spacing controls on one row on
  narrow screens. Every embedded or expanded owner preview is visual-only: no pointer action,
  keyboard stop, favorite write, slideshow, download or viewer can be triggered.
- Remove the redundant **Design**, **Contacts** and **Settings** page headings. Their real section
  headings become the first heading level inside each panel, while the existing tabs keep the
  page context visible.
- Let Gallery Settings cards use the complete panel width instead of a separate `max-width`.
- **Open full preview** is a primary black button, consistent with panel actions such as Upload.
- Add several synthetic client/photographer conversations. New comments made through a personal
  link retain the linked contact identity for photographer views. Shared client views and
  anonymous general links never expose a contact name or id. Old comments remain anonymous.
- Add richer, gallery-specific synthetic Activity examples for shared galleries, including
  sharing, favorites, comments and finished-photo delivery where the gallery state supports it.
- Gallery Contacts shows the workspace directory with search. Switches in the manage dialog only
  stage associations; **Done** applies them and **Cancel** discards them. Notes are limited to
  500 characters and fold under the row on small screens. Contacts rows share one notes/linked
  galleries layout with the `/contacts` page.
- Keep Gallery Settings autosave rules. Move the locked access-code explanation below the
  label. Copy stays a stable width; while confirmation is shown it replaces all button content
  with one centered check and ripple, then returns to Copy. The success toast remains.
- Full-image gallery cards show only the title and photo/selection summary in a thin, translucent
  glass caption. Status, lock/closed and favorite indicator remain in a row above that caption.
  In Print mode, the same row stays over the image while the caption remains below it.
- A favorite gallery keeps a permanent gold star on the dashboard. Gallery cards do not expose
  an individual favorite action; the existing bulk selection remains the only dashboard way to
  change gallery favorite state.
- The complete dashboard gallery card is one native link: clicking its image, status row or glass
  caption opens that gallery. Bulk selection remains a separate sibling control.
- On the cover inside a gallery, status stays at bottom-left and the favorite star sits
  immediately beside it. Optional dates remain below the editable title, never on the cover.
- The shared `Card` gains a `surface="glass"` mode. GalleryCard, Favorites, Final and other photo
  captions use that mode by default instead of rebuilding the visual treatment with ad-hoc
  classes. The shared surface is lighter and image-revealing, while an opaque warm fallback
  preserves readable contrast where backdrop filtering is unavailable.
- On md+ screens the Studio fills the viewport: the Journey stays on top, only the main panel
  and the side column scroll, each on its own. Selection bars become icon-only below `lg`
  (below `xl` beside the gallery rail).
- The workspace rail collapses to icons (default below 64rem, remembered per browser); secondary
  rails never collapse. Notifications keep a footer with "mark all read" and "delete all".
- Activity lines are links to the tab that holds what they describe (favorites, comments with the
  photo's thread opened, finals, contacts); a comment line shows the latest client comment as a
  quote beside one thumbnail. A failed invitation is not a link: it carries an icon-only retry
  button after its sentence. Notifications open the matching tab the same way.
- The photo viewer and client preview open and close with a fade only. The viewer header shows
  "x of y" on the left and the name centred on the photo column.
- Verify desktop and 390 px phone layouts, English/French copy, keyboard use, preview
  inactivity, explicit ZIP behavior and real comment attribution.

## Non-goals

- Do not redesign the actual client gallery.
- Do not change sharing, delivery, archiving, access-code, favorite or download authorization.
- Do not add comments to Final photos.
- Do not add comment search.
- Do not add a server-side "favorites validated" flag or authorization rule.
- Do not perform production migration or deployment.

## Approach

The chosen approach is an explicitly triggered, browser-side progress hint. A successful full
favorites ZIP download stores the exact downloaded favorite photo ids for that gallery in local
browser storage. The Journey compares that snapshot with the favorites currently loaded:

- No match: **Collect favorites** remains active.
- Matching snapshot: **Import finished photos** becomes active until a finished photo exists.
- A later favorite or unfavorite changes the set: **Collect favorites** becomes active again.

This matches how the photographer uses **Download all** without creating another lifecycle or
backend validation concept. A subset download never updates the snapshot. Server-owned ZIP
authorization, cache identity and retry behavior stay unchanged.

Two rejected alternatives were adding a persisted server validation state and treating the first
finished-photo import as proof that favorites were collected. The first expands product and
database scope; the second conflates two observable steps and skips the download signal the user
wants.

## Data and behavior constraints

- A `photo_comments.contact_id` is nullable and is set only from validated personal-link access.
  Old, general-link and photographer comments have no contact. Unlinking keeps the contact
  and its history; deleting a contact sets the identity to `NULL`.
- Comment JSON omits `contact` for client readers. Photographer responses contain only
  `{ id, name }`, never email, phone, notes or token.
- ZIP cache identity and authorization remain server-owned. The browser requests one ZIP only
  from an explicit Download action and never restores or checks a previous export when mounting.
- The full-download snapshot is browser-local convenience state only. It never controls access,
  delivery or download authorization and is ignored if local storage is unavailable or corrupt.
- Preview inactivity is structural with `inert`, not only visual `pointer-events` styling.
- Contact notes accept at most 500 characters, enforced by the application and the database.
  A migration cuts existing longer notes to 500 so every stored note still reads back.

## Verification

Run `pnpm --dir web verify`; run the focused Playwright gallery scenarios at desktop and phone
widths with the synthetic fixture; check the two gallery card modes, dashboard favorite reveal,
Studio cover placement, equal tile captions, balanced Design actions, fixed Copy confirmation,
all five Journey transitions, reload persistence of a matching favorites download and invalidation
after the favorite set changes; run the database/API checks for the new migration and comment
attribution; and inspect `git diff --check`. The cumulative preview remains uncommitted.
