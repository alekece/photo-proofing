# Wire gallery Archive and Delete to the API

Issue: [#238](https://github.com/alekece/photo-proofing/issues/238)

## Intent

The gallery page's Archive and Delete dialogs always fail with a "later release" error although the API (`POST /api/v1/galleries/{id}/archive`, `DELETE /api/v1/galleries/{id}`) and the client helpers `archiveGallery` / `deleteGallery` exist.

## Acceptance

- Confirming Archive archives the gallery and the page shows the archived state, as after Restore.
- Confirming Delete deletes the gallery and returns to the dashboard.
- A 401 goes to sign in; other failures show an error in the dialog.
- Stub comments mentioning #73 are removed. Deliver (#71) is out of scope.

## Approaches

1. Handle each action inline in the page, like `restore` and `share`.
2. Extract a shared action runner into a module.

## Chosen

Approach 1: `archive` and `remove` in `+page.svelte` call the existing helpers. Errors are rethrown as messages so the confirm dialog shows them and stays open; a 401 redirects first. Archive merges the returned `states`/`updated_at` into the page state and toasts "Archived". Delete cancels pending debounced saves (they would hit a deleted gallery) and navigates to `/dashboard`. A tiny local `failAction` helper avoids duplicating the 401 handling.
