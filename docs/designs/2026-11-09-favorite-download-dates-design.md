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

## Constraints

- A client favoriting a photo that is already a favorite must not move its date or emit an event; unfavoriting clears the date.
- Captions must fit the smallest tiles (two columns on a phone, about 170 px wide).
- Every user-visible string is a Paraglide message in both `web/messages/en.json` and `fr.json`.
- `docs/SPECIFICATIONS.md` says nothing about dates today (FR "Client activity indicators" only covers counts), so it needs one added sentence; see Decisions. Product wording needs maintainer approval.

## Approaches

1. **Keep `favorite` and add `favorited_at` beside it (rejected).** Smallest migration, but two columns can disagree (flag set, no date) and the triggers and every `AND favorite` query keep two sources of truth.
2. **Replace the flag by `favorited_at` (chosen, what the issue asks).** One column says both "is it a favorite" and "since when", exactly like `downloaded_at`. Costs a wider change: every `favorite` read and write in the repository, export, notifications, worker fixtures and tests moves to `favorited_at IS NOT NULL`.
3. **Derive the date from an events log (rejected).** Nothing records toggles today; a new table is too much for a caption.

### Where the caption lives

- **A. Overlay pill on the photo (rejected).** The bottom corners already hold the downloaded icon and the hover download button; a ~20-character caption would collide with them and cover the picture on 170 px tiles.
- **B. A short date only on the tile, full text in a tooltip (rejected).** Drops the year and time the issue requires, and hover does not exist on touch.
- **C. A caption row under the photo (chosen).** Icon plus localized date and time in small muted text, below the image inside the grid item. It never covers the photo or the buttons, wraps to two lines if needed, and masonry columns already tolerate varying heights.

## Decisions

- **Migration `0024_photo_favorite_dates.sql`:** drop the two favorite triggers first (so the backfill emits no events), add `favorited_at timestamptz`, set it to `created_at` where `favorite` was true (best available date), drop `favorite`, recreate the triggers on `favorited_at`:
  - update trigger: `AFTER UPDATE OF favorited_at ... WHEN ((OLD.favorited_at IS NULL) <> (NEW.favorited_at IS NULL))`, so a flip fires and a re-favorite does not;
  - delete trigger: `WHEN (OLD.favorited_at IS NOT NULL)`.
  `notify_favorite_event()` itself does not change.
- **Toggle:** `set_favorite` keeps its signature and its one conditional `UPDATE`: `SET favorited_at = CASE WHEN $4 THEN now() ELSE NULL END ... WHERE (favorited_at IS NOT NULL) IS DISTINCT FROM $4`. Repeats and races match no row, so the date stays and no event fires. *My call:* the `Photo` struct and JSON replace `favorite: bool` with `favorited_at: Option<Timestamp>`; no derived boolean is kept, so there is a single source.
- **Other SQL:** `favorites`, the export snapshot, the gallery favorite count and notifications' `selected` switch from `favorite` to `favorited_at IS NOT NULL`; the `COLUMNS` lists and the `false AS favorite` placeholder row become `favorited_at`. Worker and test fixtures that insert a `favorite` value follow.
- **Front schema:** `photoSchema` swaps `favorite: boolean` for `favorited_at: nullable string` (optional like `downloaded_at`); `FavoritesPanel` filters on it being non-null.
- **Formatting:** add `formatDateTime(iso)` to `web/src/lib/gallery/format.ts`, using `Intl.DateTimeFormat(DATE_LOCALES[getLocale()], { dateStyle: 'medium', timeStyle: 'short' })` in the browser's time zone, like the other helpers there.
- **Tile:** `PhotoTile` gets an optional `caption` prop (`{ kind: 'favorited' | 'downloaded', at: string }`); `PhotosPanel` passes the download date on Finals, `FavoritesPanel` the favorite date. The row shows a lucide `Heart` or `Download` icon (decorative) and the date. The existing round downloaded badge on the photo is removed on Finals, since the caption now carries the icon. *My call.*
- **Messages:** add `tile_favorited_at` ("Favorited {date}") and `tile_downloaded_at` ("Downloaded {date}") as the accessible label of the row, in en and fr; the visible text stays the bare date. `tile_downloaded` is removed if unused.
- **Spec:** with approval, add to "Client activity indicators": "Photo tiles show when the client favorited a photo and when they first downloaded a finished photo." This is a requirements change, so it waits for the maintainer.

## Testing

- **Migration regression test** in `crates/database`, using the transaction-scoped schema pattern (`CREATE SCHEMA`, `SET LOCAL search_path`, rolled back; see the tests in `gallery.rs` around line 784 and `jobs.rs` around 1168): build the schema up to `0023`, insert favorite and non-favorite photos, apply `0024`, then check that `favorited_at` equals `created_at` for former favorites and is NULL otherwise, that `favorite` is gone, and that the backfill emitted no notification.
- **Repository tests** (extend the existing favorite tests): setting sets the date; a repeat keeps the first date; clearing sets NULL; events fire on flips only, including no event on re-favoriting.
- **Front tests:** `format.ts` test for both locales; `PhotoTile` renders the caption with its accessible label; `PhotosPanel` shows the download date on Finals only for downloaded photos; `FavoritesPanel` shows the favorite date; schema tests and fixtures move to `favorited_at`.
- Run the `crates/` and `pnpm --dir web verify` checks.

## Left to the builder

Exact caption type size, colour and spacing within `docs/designs/ui/DESIGN.md`, the select-mode ring behaviour with the caption row, and the order of the migration's statements as long as the backfill emits no events.
