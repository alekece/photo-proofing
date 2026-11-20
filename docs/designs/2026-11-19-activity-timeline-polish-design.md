# Activity timeline polish and paging

Issue: [#406](https://github.com/alekece/photo-proofing/issues/406).
Builds on:
[activity log](2026-11-17-activity-log-design.md),
[activity notifications](2026-11-08-activity-notifications-design.md).

## Intent

The activity timeline remains a chronological history but becomes easier to scan and handles large
favorite selections without a hard thumbnail limit or a manual Load more control.

## Goals

- Bold the client/contact name in every attributed line.
- Add a recognizable icon for open, favorite, unfavorite, comment, download and email.
- Show up to 10 favorite-photo thumbnails per collapsed activity line, or 6 on phones.
- Expand larger lists with a chevron in the same entry.
- Load the next activity page automatically when the user approaches the list end.
- Preserve live merging and already loaded pages.

## Non-goals

- Adding a per-photo detail overlay or analytics dashboard.
- Changing activity grouping or 90-day retention.
- Replacing server-side filters or cursor paging.

## Approaches

1. **Always show every photo.** Rejected: a selection can make one line dominate the whole timeline.
2. **Keep a fixed 4-photo limit.** Rejected by the audit: it hides useful activity.
3. **Show two rows of thumbnails with an inline expansion control.** Chosen.
4. **Replace Load more with an Intersection Observer sentinel.** Chosen. It preserves the existing
   API cursor and reuses the existing loading state.

## Decisions

- Client names are bold and anonymous clients use the existing localized label.
- An action icon appears before the line text; gallery and timestamp remain secondary.
- Favorite/unfavorite events retain distinct photo IDs in first-seen order. IDs discarded from
  historical lines cannot be recovered. Other actions keep their existing four-thumbnail sample.
- The first 10 photos are visible on larger screens; on phones, the first 6 are visible. Only a
  line with more photos has a chevron, following the Contacts disclosure pattern without count text.
- The chevron opens the remaining photos in the same grid and collapses only that line.
- Scroll paging starts before the end and avoids duplicate requests while `loadingMore` is active.
- Paging starts when the user is within `400 px` of the end.
- A manual Load more button is removed.
- A failed page offers Retry at the list end without clearing existing lines.
- Live refresh continues to merge by activity line ID and keep older loaded pages.

## Open questions

1. ~~**Load threshold:** paging distance?~~ **Resolved:** `400 px` before the end.
2. ~~**Expand label:** text or icon?~~ **Resolved:** chevron/collapse pattern shared with Contacts;
   no +N text.
3. ~~**Expanded layout:** grid or horizontal strip?~~ **Resolved:** continue in the same grid.
4. ~~**Thumbnail limit:** 10 or 6 on phones?~~ **Resolved:** 10 on larger screens, 6 on phones.
5. ~~**Paging failure:** sentinel Retry or top-level alert?~~ **Resolved:** provide Retry at the sentinel.

## Verification

- Lines at or below the 10/6 threshold show all thumbnails without a chevron.
- Above that threshold, the chevron toggles the remaining thumbnails in the same grid.
- Expansion/collapse is keyboard accessible and no entry height jumps unexpectedly.
- Scrolling loads the next cursor once and not again until loading completes.
- Filtering and live refresh preserve expanded/collapsed state where practical.
