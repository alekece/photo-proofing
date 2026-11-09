# Favorite and download dates on photo tiles

Issue: [#296](https://github.com/alekece/photo-proofing/issues/296). Decisions marked *my call* are the author's and open to change in review.

## Intent

The photographer sees that a photo is a favorite or was downloaded, but not when. Show the date on the tile, always visible, so the photographer can tell fresh client activity from old.

## Goals

- Finals tiles show the download date next to the existing "downloaded" icon; Favorites tiles show the favorite date next to a heart. Both include year and time and are localized, e.g. `♥ Nov 9, 2026, 2:38 PM` (English) or `♥ 9 nov. 2026, 14:38` (French). No hover needed.
- `photos.favorite boolean` becomes a nullable `favorited_at timestamptz` (NULL = not a favorite), like `downloaded_at`. Existing favorites are kept.
- `downloaded_at` and `favorited_at` reach the front end through the photo payload.
- Favorite change events still fire only when the favorite state flips, never on re-favoriting.

## Non-goals

- A history of favorite toggles; the date is the latest time the photo became a favorite.
- Showing dates to the client (the shared gallery only needs favorite ids).
- Notification or activity-count changes.
- Thumbnail quality. The maintainer finds thumbnails too low quality; that is a separate follow-up, not part of #296.

## Constraints

- A client favoriting a photo that is already a favorite must not move its date or emit an event; unfavoriting clears the date.
- The caption must stay legible and clear of the tile controls, including on the smallest tiles (two columns on a phone, about 170 px wide); tiles may be enlarged if needed.
- Every user-visible string is a Paraglide message in both `web/messages/en.json` and `fr.json`.
- `docs/SPECIFICATIONS.md` says nothing about dates today (FR "Client activity indicators" only covers counts), so it needs one added sentence; see Decisions. Product wording needs maintainer approval.

## Approaches

1. **Keep `favorite` and add `favorited_at` beside it (rejected).** Smallest migration, but two columns can disagree (flag set, no date) and the triggers and every `AND favorite` query keep two sources of truth.
2. **Replace the flag by `favorited_at` (chosen, what the issue asks).** One column says both "is it a favorite" and "since when", exactly like `downloaded_at`. Costs a wider change: every `favorite` read and write in the repository, export, notifications, worker fixtures and tests moves to `favorited_at IS NOT NULL`.
3. **Derive the date from an events log (rejected).** Nothing records toggles today; a new table is too much for a caption.

### Where the caption lives

- **A. Overlay on the photo, bottom-left, on a subtle scrim (chosen by the maintainer).** The date reads as part of the tile and adds no height. Tiles may grow to make room, so width is not a hard constraint.
- **B. A caption row under the photo (rejected).** Never covers the picture, but separates the date from the photo and the maintainer prefers the overlay.
- **C. A short date only, full text in a tooltip (kept as fallback, see Open point).** Drops the year and time, and hover does not exist on touch.

## Decisions

- **Migration `0024_photo_favorite_dates.sql`:** drop the two favorite triggers first (so the backfill emits no events), add `favorited_at timestamptz`, set it to `created_at` where `favorite` was true (best available date), drop `favorite`, recreate the triggers on `favorited_at`:
  - update trigger: `AFTER UPDATE OF favorited_at ... WHEN ((OLD.favorited_at IS NULL) <> (NEW.favorited_at IS NULL))`, so a flip fires and a re-favorite does not;
  - delete trigger: `WHEN (OLD.favorited_at IS NOT NULL)`.
  `notify_favorite_event()` itself does not change.
- **Toggle:** `set_favorite` keeps its signature and its one conditional `UPDATE`: `SET favorited_at = CASE WHEN $4 THEN now() ELSE NULL END ... WHERE (favorited_at IS NOT NULL) IS DISTINCT FROM $4`. Repeats and races match no row, so the date stays and no event fires. *My call:* the `Photo` struct and JSON replace `favorite: bool` with `favorited_at: Option<Timestamp>`; no derived boolean is kept, so there is a single source.
- **Other SQL:** `favorites`, the export snapshot, the gallery favorite count and notifications' `selected` switch from `favorite` to `favorited_at IS NOT NULL`; the `COLUMNS` lists and the `false AS favorite` placeholder row become `favorited_at`. Worker and test fixtures that insert a `favorite` value follow.
- **Front schema:** `photoSchema` swaps `favorite: boolean` for `favorited_at: nullable string` (optional like `downloaded_at`); `FavoritesPanel` filters on it being non-null.
- **Formatting:** add `formatDateTime(iso)` to `web/src/lib/gallery/format.ts`, using `Intl.DateTimeFormat(DATE_LOCALES[getLocale()], { dateStyle: 'medium', timeStyle: 'short' })` in the browser's time zone, like the other helpers there.
- **Tile:** `PhotoTile` gets an optional `caption` prop (`{ kind: 'favorited' | 'downloaded', at: string }`); `PhotosPanel` passes the download date on Finals, `FavoritesPanel` the favorite date. The caption is overlaid at the bottom-left: a lucide `Heart` or `Download` icon (decorative) plus the date, light text on a subtle bottom gradient scrim so it stays legible on any photo. The scrim and text ignore pointer events, so the photo stays clickable.
- **No collisions:** the existing controls are the select checkbox (top-left), the delete or hero-star button (top-right), the hover/touch download button (bottom-right, 32 px) and, for unfinished photos only, a progress bar or status label along the bottom edge. The caption shows on ready photos only, stays left of the download button (it truncates before reaching it, leaving about 40 px on the right) and uses no top corner.
- **Finals badge:** the separate round "downloaded" badge (bottom-left) is removed on Finals; the overlay caption's icon replaces it in the same corner, so there is one indicator, not two. *My call.*
- **Thumbnails:** the maintainer finds thumbnails too low quality. That is out of scope here; file a follow-up issue.
- **Spec:** with approval, add to "Client activity indicators": "Photo tiles show when the client favorited a photo and when they first downloaded a finished photo." This is a requirements change, so it waits for the maintainer.

## Testing

- **Migration regression test** in `crates/database`, using the transaction-scoped schema pattern (`CREATE SCHEMA`, `SET LOCAL search_path`, rolled back; see the tests in `gallery.rs` around line 784 and `jobs.rs` around 1168): build the schema up to `0023`, insert favorite and non-favorite photos, apply `0024`, then check that `favorited_at` equals `created_at` for former favorites and is NULL otherwise, that `favorite` is gone, and that the backfill emitted no notification.
- **Repository tests** (extend the existing favorite tests): setting sets the date; a repeat keeps the first date; clearing sets NULL; events fire on flips only, including no event on re-favoriting.
- **Front tests:** `format.ts` test for both locales; `PhotoTile` renders the overlay caption with its accessible label and not on unfinished photos; `PhotosPanel` shows the download date on Finals only for downloaded photos; `FavoritesPanel` shows the favorite date; schema tests and fixtures move to `favorited_at`.
- Run the `crates/` and `pnpm --dir web verify` checks.

## Open point

Whether the full date (year and time) fits on a real tile. The builder checks it on real tiles (phone and desktop, English and French) and picks, in this order: enlarge the tiles, shorten the format (for example drop the year for the current year), or show only the icon with the full date as the tooltip (`title`) and accessible name. Record the choice in the PR.

## Left to the builder

Exact caption type size, scrim strength and spacing within `docs/designs/ui/DESIGN.md`, and the order of the migration's statements as long as the backfill emits no events.
