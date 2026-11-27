# Activity

What clients did, across every gallery. Route `/activity`, reached from the **Activity** pill in
the top `AppHeader`. The same timeline backs the gallery studio's **Activity** tab, which is
scoped to one gallery. The tab is disabled until the gallery is shared
(tooltip `"Available once the gallery is shared"`). Designs: [activity log](../../2026-11-17-activity-log-design.md)
and [timeline polish](../../2026-11-19-activity-timeline-polish-design.md).

## Layout

1. **Header:** `AppHeader` with **Activity** active, a Georgia heading **Activity**, and a short subtitle.
2. **Search and filters:**
   - Search by gallery or visitor name (including **Anonymous client**) across the workspace, or by
     visitor name within the current gallery. Partial names match regardless of case and accents.
     Search applies before pagination and combines with the fixed filters and sort.
   - **Date** offers **Any time**, **Last 24 hours**, **Last 7 days**, and **Last 30 days**.
   - **Type** pills with `aria-pressed`: **All**, **Favorites**, **Comments**, **Downloads**, **Views**, **Emails**.
   - Changing a filter reloads the first page. **Favorites** shows favorites only; removed favorites appear under **All**.
3. **Timeline:** day sections headed **Today**, **Yesterday**, or a long date; newest first by
   default, with an oldest-first sort option. A decorative action icon precedes the line, and the
   actor is bold wherever their name appears in the sentence. Gallery links (workspace only) and
   relative timestamps stay secondary. Same-kind repeats by one actor inside the window are one
   line with a count.
4. **Photos:** favorites and removed favorites show distinct thumbnails in recorded order, wrapping
   across the full row. Initially show up to 10 thumbnails on larger screens or 6 on phones. A
   chevron appears above the grid only when there are more, so it stays in place and retains focus
   as the grid expands. Other actions retain their four-photo sample. Older favorite lines cannot
   recover photos that were not recorded.
5. **Paging:** the next page loads automatically within 400 px of the list end. A failed page
   leaves the visible lines in place and offers **Retry** at the end of the list.
6. **States:** loading text, empty (`"No activity yet"` with a short explanation, or `"Nothing
   matches these filters."`), and a separate error notice for first-page or refresh failures.

## Live updates

The page refreshes when a notification event arrives (and after an SSE resync). It merges by
line ID, keeping loaded pages and expanded lines while new or extended entries appear in place.
A failed refresh leaves the list as is.

## Retention

Lines are kept 90 days after their last event by default (`[cleanup] activity_retention`) and are
removed with their gallery. The workspace page leaves out archived galleries; their own tab keeps them.
