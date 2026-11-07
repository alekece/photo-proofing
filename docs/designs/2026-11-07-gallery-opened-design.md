# Record when a client opens a shared gallery

Status: the maintainer authorised implementing right away; the PR is the review gate. Decisions marked *my call* are the author's and open to change in review.
Issue: [#211](https://github.com/alekece/photo-proofing/issues/211). The UI that shows it belongs to #72.

## Intent

The photographer cannot tell whether the client has seen a shared gallery. The API records the first time a client really opens it, so later work (#72: card label, notices) can read it.

## Goals

- A client opening the shared page tells the API once per page load.
- The first open is stored on the gallery and returned to the photographer.
- Reloads and repeat visits change nothing and send no further notice.
- Only a real client view counts. The photographer's preview, and unknown, unshared, expired or archived links, do not.
- The share token never appears in logs or spans.

## Non-goals

- Any dashboard UI, notification table or event on the photographer stream (#72). *My call:* the stored value is enough for #72 to build on; adding an event now would invent a contract before the notification model exists.
- Counting visits, tracking latest open, or identifying the client.

## Approaches

1. **Page loader records the open (rejected).** The server-side load of `/g/{token}` would call the API. It also runs for link-preview bots and prefetchers, which would report a view nobody had.
2. **Explicit call from the browser (chosen).** After the page is shown in a browser, it sends `POST /api/v1/galleries/shared/{token}/open`. Crawlers that do not run scripts do not count. The photographer's preview is the dashboard overlay of `ClientGallery` with no token and does not use `/g/[token]`, so it never calls it.
3. **Record on any shared read (rejected).** Every `GET` of the gallery, photos or thumbnails would count. That includes reloads of media and requests that are not a page view.

## Decisions

- **Stored value:** nullable `galleries.opened_at timestamptz`, set once to the database time of the first open and never changed. *My call:* first open only, as the maintainer suggested; a latest-open value needs a write per reload or a throttle, and nothing asks for it yet. It can be added later without changing this column.
- **Idempotency:** one conditional `UPDATE ... WHERE opened_at IS NULL`. Repeats and concurrent opens match no row, so the first time wins and nothing is rewritten.
- **Availability:** the route takes the existing `ClientAccess` extractor, so unknown, malformed, unshared, archived and expired links get the same 404 as every client route. The update repeats the archive and expiry conditions so a link that expires in between does not count.
- **Response:** always `204` for an available link, whether or not this call was the first, so the client learns nothing about earlier opens. `no-store` and `no-referrer` like the other client routes.
- **Photographer read:** `opened_at` is added to the gallery JSON (`null` until opened). It is not part of `SharedGallery`, so clients never see it.
- **Client ping:** `+page.svelte` of `/g/[token]` sends it once on mount, only when the gallery is available, ignoring failures: a missed ping must never affect viewing. *My call:* a same-origin `fetch` with `keepalive`, like the selection calls.
- **Secrets:** the token is in the path only. No new log, span or `#[instrument]` is added; the API has no access logging today, so the criterion holds because nothing logs the path; the existing doc rule that any access log must redact `/galleries/shared/` covers the new route. Reverse-proxy logs are deployment configuration outside this change.
- **Not retroactive:** existing galleries start with `null`. Past opens were never observed, so nothing is backfilled.

## Testing

- Database: first open sets `opened_at`; a second open keeps the first value; expired and archived galleries are not marked; the column is `NULL` for new galleries.
- API: the route returns 204 and sets `opened_at` once (visible in the photographer's gallery JSON); unknown, expired and archived links return the common 404 and record nothing; the photographer's own gallery read does not set it.
- Web: the preview overlay (`ClientGallery`) sends no ping; the page sends one `POST` when a gallery is shown and none when it is unavailable; a failing ping does not break the page.

## Limitations

- A photographer who opens the shared link in their own browser (not the preview) counts as a client; the API cannot tell them apart.
- A client who opens the link in a browser that runs scripts counts, even if it is an automated one. Telling people from such tools is out of scope.
- The open is not announced live; #72 decides how the photographer is told.
