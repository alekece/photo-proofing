# Select and delete several photos at once

Issue: #146. Requirements: FR-05 (photo removal); builds on the
[Photos tab upload](2026-11-04-photo-upload-ui-design.md) (#102) design and its delete route. Third
layer of a stack with #80 and #102 below.

## Intent

After a large upload, the photographer can clear out many photos in a few clicks instead of
confirming each one.

## Goals

- A select mode on the Photos tab with select all and one confirmation for the whole selection.
- One request deletes the selection, with the same guarantees as deleting a single photo.

## Non-goals

- Selecting files still in the browser's upload queue: their ✕ already removes them at once.
- Moving, reordering or downloading a selection.
- The warning for favorited photos (#68).

## Anti-goals

- Deleting photos of another gallery or workspace through crafted ids.
- A half-applied selection that leaves the page unsure what was deleted.

## Approaches

1. **One bulk request (chosen).** `POST /galleries/{id}/photos/delete` with the ids; one
   transaction, one answer. Cost: a second route next to the single delete.
2. **One `DELETE` per photo from the browser.** No new route, but hundreds of requests for a large
   selection and partial failures to report.

## Decisions

Decided with the product owner:

- Select mode with a selection bar and a select-all shortcut; deleting the selection asks for
  confirmation once.
- Its own issue, after #102.

My calls (to check while reading):

- **Select mode.** A `Select` button on the Photos tab toggles it. In select mode, server photos
  (any status) show a checkbox and clicking a tile toggles it instead of opening the viewer; ✕ and
  Retry are hidden. Queued browser files are not selectable.
- **Selection bar.** "n selected · Select all · Delete · Cancel". Select all (or Ctrl/⌘+A in
  select mode) selects the server photos shown at that moment; photos that arrive later are not
  added, so the confirmed count is exactly what is sent. Esc leaves select mode and clears the
  selection. Delete is disabled with nothing selected and opens the existing `Dialog`: "Delete n
  photos? This can't be undone."
- **Route.** `POST /api/v1/galleries/{id}/photos/delete` with `{"photo_ids": [...]}` (session and
  ownership), answering `{"deleted": n}`. Ids outside this workspace and gallery, or already gone,
  are ignored rather than failing the request, so a selection racing another tab still succeeds;
  a repeated id counts once. An empty list or more than 1000 ids is 400; a body that is not a
  list of UUIDs is rejected like any JSON body (400 when malformed, 422 for a wrong type), with a
  JSON limit sized for 1000; an unknown gallery is 404. It shares the database
  delete and file clean-up of the single delete: jobs and rows go in one transaction, files are
  removed after commit.
- **Browser.** Selected tiles disappear at once. A selection larger than 1000 is sent in chunks,
  one after another. If a chunk fails, the rest are not sent, the list is refetched so the page
  shows exactly what the server kept, select mode stays open with the surviving photos still
  selected, and the panel's alert says how many were deleted. #80's `deleted` events reconcile any other
  open tab.
- **Literal route before `{photo_id}`.** `/photos/delete` is registered before the `{photo_id}`
  routes, as `/photos/retry` already is.

## Left to the builder

- Bar placement and styling within the mockup tokens; whether Ctrl/⌘+A also enters select mode.
- Name of the shared delete function.

## Verification

API tests: bulk delete removes the given photos, their jobs and files; ids from another gallery or
workspace and unknown ids are untouched and not counted; over 1000 ids is 400; unknown gallery is
404; needs a session. Web tests: entering and leaving select mode, select all, Ctrl/⌘+A, Esc,
confirmation, optimistic removal and restore on failure, queued files not selectable; axe passes.
