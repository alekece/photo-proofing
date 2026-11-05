# Validate API responses and the sort parameter

Issue: #154.

## Intent

If the API returns something the web app doesn't expect, or someone edits the URL by hand, the
page shows a clean error or a sensible default instead of breaking in odd ways.

## Goals

- Every JSON body the web app reads from the API is checked before use:
  - server loads: `readGalleries`, `readShared`;
  - browser clients in `lib/gallery/api.ts` and `lib/gallery/photos.ts`;
  - live `photo` events in `lib/events.ts`.
- An invalid body is a controlled failure:
  - **Server load:** a 503, handled like an unreachable API. The dashboard keeps its inline
    "Couldn't load" state; other pages show the error page from #152.
  - **Browser call:** what the caller already handles for that endpoint. The gallery, photo
    list, bulk delete and upload limits calls throw `GalleryApiError`. An upload returns
    `{ kind: 'failed' }`, since uploads only throw on abort.
  - **Live event:** ignored, then a refetch, as a `resync` does.
- An unknown dashboard `sort` falls back to the default order, on the server and in the browser.

## Non-goals

- Validating request bodies the web app sends; the API validates those.
- Sharing a schema with the Rust API or generating types.

## Approach

Small hand-written type guards in `web/src/lib/gallery/validate.ts`, next to the types they
check. No new dependency: the shapes are few and flat, and a library like zod would add weight to
the client bundle for little gain.

- Each guard checks only fields the web app uses, with their types and allowed string values
  (layout, sort, photo status). Extra fields are allowed, so the API can grow without breaking
  the web app.
- `readGalleries` and `readShared` throw SvelteKit's `error(503)` when a guard fails.
- Browser clients fail in each endpoint's existing way, as listed above, so current UI paths
  handle it.
- `isGallerySort` replaces the cast in the dashboard's `changeSort`. The server already falls back
  to `'updated'`; it switches to the same guard.
- `events.ts` checks `photo` event payloads. An invalid one triggers the resync handler, so the
  page refetches rather than trusting it. `isSignedIn` stops casting the status body.

## Testing

Unit tests cover:
- each guard: valid shapes, missing fields and wrong types;
- every JSON success path gives its controlled failure on a malformed body: `readGalleries`,
  `readShared`, the gallery requests, `listPhotos`, `deletePhotos`, `getUploadLimits` and
  `uploadPhoto`;
- an invalid live event leads to a refetch;
- an invalid `sort` gives the default order, both in the server load and in the browser handler.
