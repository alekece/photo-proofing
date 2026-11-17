# Activity

What clients did, across every gallery. Route `/activity`, reached from the **Activity** pill in
the top `AppHeader`. The same timeline backs the gallery studio's **Activity** tab, which is
scoped to one gallery and has no Gallery filter. The tab is disabled until the gallery is shared
(tooltip `"Available once the gallery is shared"`). Design: [activity log](../../2026-11-17-activity-log-design.md).

## Layout

1. **Header:** `AppHeader` with **Activity** active, a Georgia heading **Activity**, and a short subtitle.
2. **Filters:**
   - **Gallery**, **Visitor** (linked contacts, plus **Anonymous client** for the general link) and
     **Date** (**Any time**, **Last 24 hours**, **Last 7 days**, **Last 30 days**) selects.
   - **Type** pills with `aria-pressed`: **All**, **Favorites**, **Comments**, **Downloads**, **Views**, **Emails**.
   - Changing a filter reloads the first page. **Favorites** shows favorites only; removed favorites appear under **All**.
3. **Timeline:** day sections headed **Today**, **Yesterday**, or a long date; each an ordered list of
   lines, newest first. A line shows up to four photo thumbnails, who did what ("Anna favorited 3
   photos", "Anonymous client opened the gallery", "Invitation email sent to Anna"), the gallery
   title as a link to `/dashboard/{id}` (workspace page only) and how long ago. Same-kind repeats by one
   actor inside the window are one line with a count.
4. **Load more** appends the next page.
5. **States:** loading text, empty (`"No activity yet"` with a short explanation, or `"Nothing matches these filters."`), and an
   error notice (`role="alert"`) when a load fails.
6. **Phone (`390 px`):** filters wrap onto several rows; lines stack the thumbnails under the text.

## Live updates

The page refreshes its first page when a notification event arrives (and after an SSE resync). The
refresh merges by line id, so new or extended lines appear in place and nothing already shown
blinks or disappears; pages loaded with **Load more** are kept. A failed refresh leaves the list as is.

## Retention

Lines are kept 90 days after their last event by default (`[cleanup] activity_retention`) and are
removed with their gallery. The workspace page leaves out archived galleries; their own tab keeps them.
