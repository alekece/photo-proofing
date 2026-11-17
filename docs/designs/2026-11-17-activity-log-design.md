# Workspace activity log and gallery Activity tab

Issue: [#350](https://github.com/alekece/photo-proofing/issues/350). Requirements: FR-02, FR-11, FR-12, NF-01, TC-05. Builds on the
[activity notifications](2026-11-08-activity-notifications-design.md) (the bell, kept), the
[contacts directory](2026-11-13-contacts-directory-design.md) (attribution), the
[sharing invitations](2026-11-14-gallery-sharing-invitations-design.md) (emails) and the studio layout
([#342](https://github.com/alekece/photo-proofing/issues/342)). The mailing side of
[#349](https://github.com/alekece/photo-proofing/issues/349) (expiration reminders) only has to write
`email` rows into the table defined here. The page layout is the mockup validated in the issue. Decisions
marked **my call** are mine and are for review to confirm.

## Intent

The photographer sees, in one chronological place, what each client did across galleries and who did it,
without watching the bell. The bell stays the quick, grouped "what is new" entry point; Activity is the
history to browse and filter.

## Goals

- A workspace page `/activity` and a gallery Activity tab showing day-grouped lines for gallery opens,
  favorites, removed favorites, comments, downloads and sent emails.
- Each line is attributed to the contact who used their personal link, or to "Anonymous client" for the
  general link.
- Rapid repeats collapse into one line ("favorited 20 photos"), not 20 rows.
- The bell keeps its meaning (FR-12) and is fed from the same activity, so the two can never disagree.
- Filters by gallery, visitor, date and type; the page updates live without flicker.

## Non-goals

Photographer-side actions (edits, uploads, replies), per-photo analytics, export, email or push, a
calendar date picker, and any change to what the bell shows or how it is read.

## Approaches

1. **Triggers write the log.** Covers every writer, but a trigger cannot know which link was used, so the
   contact would travel through a transaction-local setting, a hidden coupling. Rejected.
2. **Repository code writes the log in the same transaction, with an explicit client (chosen).**
   Attribution is typed and visible, and the bell derives from the log. Cost: every client action must
   record, which tests have to prove.
3. **Derive the log from existing timestamps.** No contact, no history, no comments or emails. Rejected.

## Decisions

- **Storage.** `activity_events(id, workspace_id, gallery_id → galleries ON DELETE CASCADE, contact_id →
  contacts ON DELETE SET NULL, kind, count, photo_ids, detail, first_at, last_at)`. `kind` is `open`,
  `favorite`, `unfavorite`, `comment`, `download` or `email`. `photo_ids` holds the first few photos for the
  thumbnails (**my call**: at most 4), `detail` the quote snippet of a comment or the email template
  (invitation or reminder). Indexes on `(workspace_id, last_at DESC, id DESC)` and
  `(gallery_id, last_at DESC, id DESC)`. A contact deleted later turns its lines into "Anonymous client" and
  never removes them.
- **One line per window.** A new event extends the row of the same gallery, contact (NULL included) and kind
  whose `last_at` is within `PP_ACTIVITY_WINDOW` (a human-readable duration, default 30 minutes), adding to
  `count` and `photo_ids` and moving `last_at`. Otherwise it opens a row. A sliding window has no unique
  key, so the write takes a transaction-scoped lock on gallery, contact and kind first (**my call**), and
  concurrent events neither lose counts nor split a window. `count` counts changes, as the existing
  notification does, so picking, un-picking and re-picking one photo counts three. Un-picking is its own
  `unfavorite` line, so picking then un-picking stays honest. Thumbnails of photos deleted since are left
  out when a line is read.
- **Attribution and recording.** `PhotoRepository`, `CommentRepository` and `GalleryRepository` take an
  `Option<Client>` in their `Options`, where `Client` holds `Option<ContactId>`; `None` is the photographer
  and records nothing. The client routes build it from `SharedAccess`, which already carries the contact.
  The upsert and window SQL live in one extension trait on the connection, private to the `database`
  crate; each repository uses that trait in its own transaction and no repository calls another. No free
  functions. A favorite change that changes nothing (picking a picked photo) records nothing, so
  `set_favorite` and the other single-statement writers become transactions that report whether a row
  changed; the delivery ZIP's `ExportRepository` takes the client through its options too.
- **What records what.** Gallery open (already throttled by `record_opened`); a favorite flip; a client
  comment (not edits, deletions or photographer replies); the first download of a final, and the first
  download of the delivery ZIP, which always logs one line (`photo_ids` are the photos it newly marked, the
  count may be 0), as the bell does today (**my call**); a successfully sent email, recorded by the worker
  with the recipient as contact (a BCC recipient has none and shows as "Anonymous client"; **my call**).
  Until #349 adds reminders, `detail` only holds the invitation. A retried send may log twice in the rare
  case it fails after sending, which is acceptable (**my call**). History starts empty at the migration;
  nothing is backfilled and unread notifications are kept as they are (**my call**).
- **The bell derives from the log.** One trigger on `activity_events` calls the existing
  `record_notification` with the change in `count` (**my call**: `favorite`/`unfavorite` map to the
  `selection` kind with count 0, as today). It replaces the `photos_record_selection`,
  `photos_record_download`, `exports_record_download`, `galleries_record_opened` and
  `photo_comments_record` triggers; a client `comment` still notifies with count 1, as today, and `email`
  does not. The `photos_record_selection_delete` trigger stays: deleting a
  favorited photo is the photographer's action and still notifies. The `notifications` table, its read
  state and its endpoints do not change. A migration regression test proves the bell is identical before
  and after.
- **API.** `GET /api/v1/activity` (workspace) and `GET /api/v1/galleries/{id}/activity` return lines with
  gallery id and title, contact name (or none), kind, count, photo ids, detail and the two times, newest
  first, cursor-paged on `(last_at, id)`, 30 per page. Filters are server-side: `gallery`, `contact`
  (including anonymous), `kind` and `range` (`today`, `7d`, `30d`). The workspace feed hides archived
  galleries (**my call**). Both are session-only and workspace-scoped; a foreign gallery id is 404. Contact
  names are photographer-side data only; client responses are unchanged.
- **Retention.** The worker's sweep deletes rows older than `PP_CLEANUP_ACTIVITY_RETENTION` (default 90
  days), next to the notification sweep. Archiving keeps rows; deleting a gallery cascades.
- **Live.** The trigger on `activity_events` announces the workspace and gallery (ids only) on the existing
  `notification_events` channel, so emails, which do not notify the bell, also reach the browser and no new
  channel is needed (**my call**). The Activity view refetches its first page on that signal and after a
  resync.
  The refetch must not flicker: the list is merged by id, existing rows keep their place and element, and
  there is no loading placeholder or scroll jump; a changed row updates in place, a new line slides in at
  the top of its day. Pages loaded with "Load more" are kept. While a filter changes, the previous list stays
  until the new one is ready.
- **Web.** `/activity` under `web/src/routes/activity/`, with the header "Activity" pill enabled; the gallery
  Activity tab in the studio reuses one shared timeline component, pre-scoped to the gallery and without the
  gallery filter. Filter bar and timeline follow the issue's mockup; empty state and keyboard operation
  follow the existing pages. Line text is English like the rest of the app today.
- **Docs.** `docs/SPECIFICATIONS.md` gets an activity-log requirement (the FR-12 bell is untouched),
  `docs/releases/MVP.md` its acceptance checks, and a screen note under `docs/designs/ui/screens/`.

## Left to the builder

Table and trigger names, the exact line wording, the timeline component's structure, how `PP_ACTIVITY_WINDOW`
reaches both the API and the worker (the worker writes `email` rows), and the cursor encoding.

## Verification

Database: coalescing inside and outside the window, per kind and per contact; concurrent events never lose
counts; contact deletion keeps lines; cascade on gallery deletion; the bell is unchanged (every case of the
2026-11-08 design, run again against the new derivation, plus a migration test); retention sweep. API:
every client action records one line with the right contact or none, a no-op records nothing, the
photographer's actions record nothing, listing, each filter, cursor paging, workspace isolation,
unauthenticated access, archived galleries hidden in the workspace feed. Worker: a sent email records a line
for its recipient and a failed send records none. Web: filters, load more, day grouping, empty state, and a
live update that keeps existing rows and scroll position.
