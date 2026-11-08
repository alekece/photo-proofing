# Client activity and grouped notifications

Issue: #72. Requirements: FR-02, FR-11, FR-12, NF-01, NF-02, NF-03, TC-05, AC-05. Builds on the
[client selection](2026-11-05-client-selection-design.md), the
[live events](2026-11-04-live-photo-events-design.md) and the
[client downloads](2026-11-08-client-downloads-design.md) (which records `downloaded_at`). The screens
follow [dashboard.md](ui/screens/dashboard.md). Decisions marked **my call** are mine and are for review to
confirm.

## Intent

The photographer sees what clients did without watching each gallery: the dashboard cards show where each
gallery stands, and the bell lists one unread notification per gallery with new activity.

## Goals

- Gallery cards: selected count (warning colour when above the included count, as on the client page) for
  proofing galleries; "N finals · M downloaded" or "· all downloaded" for delivered ones.
- One unread notification per gallery, accumulating until read; activity after a read opens a new one.
- Read state belongs to the workspace, survives restarts, and never changes the card counts.
- Live updates over the existing event stream, with a refetch after a reconnect.

## Non-goals

Per-client accounts, an analytics dashboard, per-photo notification rows, email or push. Claiming that a
client saved a file. Showing the "news chips" under the greeting (only its "N galleries have news" line).

## Approaches

1. **Notification rows written by triggers (chosen).** A table with at most one unread row per gallery.
   Database triggers on favorite and download changes upsert it, so every writer (API, worker, future
   code) is covered, and the existing event channels carry the live update. Costs: one table and trigger
   logic to keep correct.
2. **Derived from timestamps.** A per-gallery "read at" marker compared with activity times: no table, but
   a read and an activity inside one transaction window can lose activity, and the panel keeps no history.
   Rejected.

## Decisions

- **Storage.** `notifications(id, workspace_id, gallery_id → galleries ON DELETE CASCADE, kind
  'selection'|'download', count, created_at, updated_at, read_at)`. A partial unique index on `gallery_id`
  `WHERE read_at IS NULL` enforces one unread row per gallery. Workspace scoping is the row's own
  `workspace_id`, so read state is shared by every browser.
- **Writing (triggers, same transaction as the activity).**
  - A favorite change (the existing `photos_notify_favorite` conditions, including deleting a favorite)
    upserts the gallery's unread row as `selection`.
  - The first download of a final (`photos.downloaded_at` NULL → set) upserts it as `download` and adds 1.
    A ZIP download marks its included finals in one transaction, so it adds one per newly marked photo;
    repeated downloads change nothing, so counts never inflate.
  - The first download of the delivery ZIP (`exports.downloaded_at`) upserts the unread `download` row
    without adding to the count (**my call**), so a ZIP of already-downloaded photos is still reported.
  - If the unread row has the other kind, the kind switches and the count restarts at the new activity.
    Selection locks at delivery, so the two kinds rarely meet.
  - Each write bumps `updated_at` and announces the workspace on a new `notification_events` channel.
- **Summary text (**my call**).** The row stores facts, the text is built when shown: "13 selected" (the
  gallery's live selected count, so it never goes stale), "1 new download" / "4 new downloads", or
  "Downloaded as a ZIP" when the count is 0. Relative time comes from `updated_at`.
- **Reading.** `POST /api/v1/notifications/{id}/read` sets `read_at` where it is still NULL (idempotent);
  `POST /api/v1/notifications/read` sets it for every unread row of the workspace. Both take the usual
  `Origin` check. Opening the panel or the gallery reads nothing; the row click reads then navigates.
  Races hold because the upsert and the read both lock the unread row: activity that commits first is
  included in the read, activity after it finds no unread row and starts a new one.
- **Listing.** `GET /api/v1/notifications` returns every unread row of the workspace plus its newest 20 read rows,
  each with gallery id, title, hero id, kind, count, the live selected count, `read_at` and `updated_at`,
  plus the total unread. The sweep deletes read rows older than `PP_CLEANUP_NOTIFICATION_RETENTION` (default 30 days; **my call**); the cap and the
  retention are constants, not settings.
- **Cards.** The gallery overview gains the ready finals count and the downloaded finals count. Delivered
  cards show "N finals · M downloaded", or "· all downloaded" only when N > 0 and M = N; removed finals stop
  counting and new ones start undownloaded because both are read from the current ready finals. A card also
  shows a "New selection" / "New downloads" badge while its gallery has an unread notification.
- **Live.** `notification_events` is relayed like the other channels (ids only, workspace-scoped stream);
  the bell and the dashboard refetch on it and after a resync. The greeting says "N galleries have news"
  from the unread rows.
- **Bell.** Dot with the unread count, an accessible name that says it, a panel with one row per gallery
  (thumbnail, title, summary, time) and a **Mark all as read** link (disabled when nothing is unread),
  "Nothing new." when empty. It opens and closes by keyboard, closes on Escape and outside click, and
  works on the phone layout.

## Left to the builder

Table and index names, trigger structure, the exact payloads, the row component, and how the panel shares
the dashboard's event subscription. The text is English like the rest of the app today.

## Verification

Trigger tests: selection and download activity create or update one unread row; a read followed by
activity creates a new row; repeated downloads and ZIP-after-photos do not inflate the count; deleting a
gallery removes its rows; concurrent read and activity never lose activity. API tests: listing, read and
read, workspace isolation, unauthenticated access. Card tests for the counts and "all downloaded".
Web tests for the panel (no read on open, read on click, mark all, empty, live update) and the badge.
