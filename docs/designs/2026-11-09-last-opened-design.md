# Show when a client has opened the gallery

Issue: [#292](https://github.com/alekece/photo-proofing/issues/292). Builds on the open recording of #211 (`galleries.last_opened_at`, written by `POST /api/v1/galleries/shared/<token>/open`) and the [activity notifications](2026-11-08-activity-notifications-design.md) (#72). Decisions marked *my call* are the author's and open to change in review.

## Intent

The API already records when a client last opened the shared gallery, but the photographer sees nothing. Show the last open in the gallery hero and tell the photographer, through the existing bell, that the client has looked, without a flood of notifications.

## Goals

- The gallery hero shows `Last opened Nov 9, 2026, 2:38 PM` (French `Dernière ouverture le 9 nov. 2026, 14:38`), or `Not opened yet`, in the meta line beside the photo count and creation date.
- A client open creates an "open" notification in the bell, and a gallery card badge "Opened" while it is unread.
- Repeated opens never create more than one unread row per gallery.

## Non-goals

- Per-visit history, visitor counts or analytics; one last-open date only.
- Counting previews, or opens of unavailable (unshared, expired, archived) galleries. Nothing changes in the recording: `record_opened` already ignores archived and expired galleries, and the owner Preview does not call the open route.
- A date on the dashboard cards (the badge is enough there).
- Email or push.

## Constraints

- Agreed with the maintainer: both a label and a notification; every open counts, with the existing one-minute throttle; reuse the notification model as a new kind.
- The unread row is unique per gallery, and a kind change restarts it (`record_notification`); `open` follows the same rule as `selection` and `download`.
- Every user-visible string is a Paraglide message in `web/messages/en.json` and `fr.json`.
- `docs/SPECIFICATIONS.md` does not mention opens, so it needs a requirements change (see Decisions), which needs maintainer approval.

## Approaches

1. **Count opens on the unread row (chosen).** Same trigger-and-upsert machinery as downloads: each recorded open adds 1 to an unread `open` row, so the text can say "opened 3 times". Costs: one more kind to handle in SQL, Rust and the front. Opens follow the existing rules unchanged, so an open on an unread row of another kind switches it to `open` and restarts the count, as a download already does.
2. **One row, no count (rejected).** A flat "Opened the gallery" that only bumps `updated_at`. Simplest text, but ignores the model's habit of counting repeats and hides whether the client came back.
3. **Compare `last_opened_at` with a read marker, no notification row (rejected).** No table change, but the bell, card badge and "N galleries have news" would need a second source, and #72 already rejected derived read state for races.

## Decisions

- **Kind.** A new, independent `open` notification kind. Migration `0026_notification_opened.sql`: drop and re-add `notifications_kind_known` with `'open'`, add the `open` variant to the Rust `NotificationKind` and the front `picklist`.
- **Writing.** A trigger `AFTER UPDATE OF last_opened_at ON galleries WHEN (NEW.last_opened_at IS NOT NULL AND (OLD.last_opened_at IS NULL OR NEW.last_opened_at > OLD.last_opened_at))` calls `record_notification(workspace_id, id, 'open', 1)`. Because `record_opened` only updates past the throttle, every recorded open (including the first) adds 1 and reloads inside a minute add nothing. Any future writer is covered too.
- **Coalescing.** `open` is an independent kind. It coalesces only with its own unread row (count + 1), exactly like `selection` and `download`; `record_notification` does not change, so an open always shows in the bell.
- **Texts.** Bell row: `The client opened the gallery` / `The client opened the gallery 3 times` (count > 1), French `Le client a ouvert la galerie` / `… 3 fois`. Card badge: `Opened` / `Ouverte`. Time ago comes from `updated_at` as usual.
- **Hero label.** In `GalleryHeader`, append to the existing meta line, only for a shared gallery (an unshared gallery has no client to open it, so "Not opened yet" would mislead): `Last opened {date}` / `Not opened yet`, French `Dernière ouverture {date}` / `Pas encore ouverte`. The date uses `formatDateTime` (present since #296, localized, browser time zone). It must wrap cleanly on a phone.
- **Card.** Only the badge from the notification (no date, no extra line). It follows from the existing unread-notification badge, so a new message and one `notificationBadge` / `notificationText` branch is all it needs.
- **API and schema.** The gallery JSON already carries `last_opened_at` (Rust `Gallery`, covered by the shared-route test). The front `gallerySchema` does not parse it yet: add `last_opened_at: v.nullable(v.string())`, optional like other late-added fields, and update fixtures. The notifications endpoint needs no new field; `kind` gains a value.
- **Docs.** README no longer says the UI does not show it. With approval, add to the "Client activity indicators" FR: "The gallery page shows when the client last opened the shared gallery, or that it has not been opened yet." and to "Grouped notifications": "The top bar also reports when a client opens the gallery." This is a requirements change that waits for the maintainer.

## Testing

- **Migration regression test** in `crates/database` using the transaction-scoped schema pattern (see `gallery.rs`, `jobs.rs`): up to the previous migration, insert an existing `selection` row, apply the new one, then assert `INSERT ... kind 'open'` is accepted, an unknown kind is still rejected, and existing rows are intact.
- **Notification tests** (extend `notifications.rs`): first open creates an unread `open` row with count 1; a second open after the throttle makes it 2; an open inside the throttle changes nothing; an open on an unread selection row switches it to `open` with count 1 and a selection on an unread `open` row switches it back, like the other kinds; an open after a read starts a new row; archived or expired galleries create nothing.
- **API test:** the notifications list returns kind `open` after the open route.
- **Front tests:** schemas parse the new kind and `last_opened_at`; `notificationText` and badge for count 1 and 3 in both languages; `GalleryHeader` shows `Last opened` once opened, `Not opened yet` when shared and unopened, and nothing when unshared.
- Run the `crates/` checks and `pnpm --dir web verify`.

## Left to the builder

The migration number, trigger and function names, exact message keys, spacing and wrapping of the hero meta line, and whether the open gallery page refetches its gallery on a `notification_events` message (nice if cheap; reload is acceptable).

## Open questions

- May the PRD sentences above be added?
