# Private client link and client gallery page

Issue: #65. Depends on #63 (account sessions) and #64 (gallery configuration),
both merged. Product requirements: `docs/SPECIFICATIONS.md` FR-04 (private
sharing), FR-13 (expiry enforcement) and NF-01 (access and privacy).

## Intent

A photographer can share a gallery before any photo exists: one click creates
a private link, one click copies it. Anyone holding the link sees the
photographer's persisted gallery (title, optional expiry, layout) on a client
page, or a neutral "unavailable" page when the link is unknown or expired.

This is also the slice that fixes how every later client route (media,
favorites, ZIP, delivery, live updates) authorizes. Getting that single pattern
right matters more than the visible page.

## Goals

- Share creates an unguessable link; sharing again returns the same link. The
  link is fixed for the gallery's lifetime (user decision: no rotation).
- The client page loads persisted gallery data from the API instead of mocks,
  honors the gallery layout (uniform grid or masonry), and shows the title and
  the optional expiry. Neither a hero image nor an expiry is required.
- An empty shared gallery shows a useful empty state to the client and a
  warning to the photographer when they share it.
- Expiry is enforced on every client request, at request time.
- One reusable authorization path for client routes, so later slices add a
  route without re-deciding token handling.
- The link token does not leak through logs, page metadata or referrers.

## Non-goals and anti-goals

- No SSE or live updates. Split out to #80, because no events exist until
  uploads and processing (#66) do.
- No photographer preview of the client page. Agreed as a separate issue, #81,
  after this one; it is not in the current PRD.
- No live refresh of the empty client page: a manual browser reload is accepted.
  The normal flow is upload first, then share, so clients see all photos.
- No photos, thumbnails, hero image or ready-proof listing. Those arrive with
  #66; this slice defines no photo contract beyond what the page needs now.
- No link rotation, unshare, passcodes, client accounts, email or SMS.
- No archive, restore or deletion. They belong to #73, which will add its
  checks to the same authorization path.
- No rate limiting of link guesses; a 128-bit random token is not guessable
  online. Revisit when the site is exposed publicly.
- Anti-goal: any difference visible to a client between an unknown link, an
  expired link and (later) an archived or deleted gallery.

## Constraints

- Rust, Actix, sqlx and PostgreSQL; SvelteKit with Tailwind and shadcn-svelte.
  Follow `AGENTS.md` and the patterns of the gallery slice (#64): root-scope
  middleware and config on `/api/v1`, `ErrorResponse`, `ResponseBuilderExt`,
  repository in `pp-database`, newtypes for meaningful values.
- No client session or cookie. The token is the only credential.
- Local-network scope does not waive validation, access control,
  accessibility, or restart safety.

## Approaches

**A. Token in the URL, checked on every request (chosen).** The link is
`/g/{token}`. The SvelteKit server load calls a public API route with the
token. One extractor in the API resolves the token and checks access, and
every later client route reuses it. No client state to expire or revoke.
Trade-off: the token appears in all client URLs, mitigated by
`Referrer-Policy: no-referrer`, `noindex`, and not logging it.

**B. Exchange the token for a short-lived client cookie.** The first visit
redirects to a clean URL and sets a cookie scoped to the gallery. Keeps the
token out of history and media URLs. Rejected for the MVP: it adds cookie
lifetime, per-browser state, CSRF for client writes and a revocation re-check,
while the PRD wants no client accounts or codes and treats the link itself as
the credential.

The user picked A.

## Design

### Data model

New migration `0003_share_tokens.sql` (the gallery migration is merged, so it is
not edited in place):

- `galleries.share_token text` (nullable) with a unique index. `NULL` means the
  gallery has not been shared. The token is stored in plaintext, not hashed,
  because sharing again must be able to show the same link; the link is a
  bearer credential equivalent to a capability URL.
- A CHECK constraint on the encoded shape (exactly 22 characters from the URL-safe
  base64 alphabet) so a malformed value cannot be stored.

### Token

- `ShareToken` newtype in `pp-database`: 128 random bits from the OS-seeded
  generator, encoded as 22 unpadded URL-safe base64 characters, for example
  `Xk3vQ9mT2aLw8RbN5ycE1d`.
- Parsing accepts only the canonical 22-character encoding, so malformed path
  values are rejected before touching the database. The API's existing
  `SessionToken` already does the same for 256-bit cookie tokens; the builder
  decides whether to generalize it or write a sibling type.
- `Debug` does not print the value.

### Database crate

- `GalleryRepository::share(workspace_id, id) -> Option<ShareToken>`: sets the
  token only when it is `NULL`, atomically, so concurrent shares converge on
  one token. Returns `None` for another workspace's gallery.
- `GalleryRepository::find_shared(token) -> Option<SharedGallery>`, where
  `SharedGallery` carries only what a client may see (title, layout,
  `expires_at`). A single query that applies `expires_at IS NULL OR expires_at >
  now()` using the database clock, so unknown and expired are the same `None`.
- `Gallery` (the photographer's view) gains `share_token: Option<ShareToken>`.

### API

- `POST /api/v1/galleries/{id}/share` (authenticated, same-origin): creates the
  token if missing and returns the updated `Gallery`. Idempotent. Another
  workspace's gallery is a 404, like the other gallery routes.
- `GET /api/v1/shared/{token}` (no authentication): returns
  `{ title, layout, expires_at }` or a 404 with the standard `ErrorResponse`
  body. Unknown token, malformed token and expired gallery are byte-identical
  responses. Responses are `Cache-Control: no-store` and
  `Referrer-Policy: no-referrer`.
- The shared route parses the path token and looks the gallery up by it; no
  separate access layer exists yet. Later client routes can extract one when
  they need archive, deletion, delivery state or downloads. (Revised after PR
  review: an earlier draft had a `SharedAccess` extractor.)
- No request logging exists today, so nothing logs the token. `ShareToken`'s
  `Debug` is redacted and the route documents that any access log added
  later must redact `/shared/` paths.

### Web

- `/g/[gallery]` is renamed to `/g/[token]`; its server load calls
  `GET /api/v1/shared/{token}` and maps a 404 (or any failure to resolve) to
  the unavailable page. The page keeps `no-referrer` and `noindex` metadata.
- Delete the mock data (`lib/mock`), the client-side access mirror
  (`gallery/access.ts`) and the superseded states (`draft`,
  `selection_received`, `retouching`, `GalleryStateNotice`). The API is
  authoritative; the UI renders what it returns.
- Client page: title (with the "Untitled gallery" placeholder, shared with the
  dashboard), expiry badge when an expiry exists, and the empty state "The
  photographer hasn't added photos yet. Check back soon." Layout selects uniform
  grid or masonry for the photo area; both share the accessible viewer controls
  of the existing `ProofViewer`. Because no photos exist yet, the layouts are
  exercised in component tests with fixture photos.
- Unavailable page: one neutral message that says the gallery is not available
  and to contact the photographer. It is used for every denial (user decision,
  so that an expired link is indistinguishable from an unknown one).
- Photographer gallery page (`/dashboard/[id]`): a Share control. First click
  calls the share route and shows the link in a read-only field with a Copy
  button; an already shared gallery shows the link immediately. Copy uses the
  Clipboard API and falls back to selecting the field. While the gallery has no
  photos (always, until #66) a warning says clients will see an empty gallery.
  The link is built from the web origin the photographer is using.

### Testing

- Database: share creates once and is idempotent, concurrent shares converge,
  another workspace cannot share, `find_shared` returns nothing for unknown,
  expired and unshared, and honors no-expiry and future expiry; the CHECK
  constraint rejects malformed tokens.
- API: share is authenticated and same-origin; shared lookup succeeds for a valid
  token; unknown, malformed and expired links produce identical status, headers
  and body; `no-store` and `no-referrer` are present; cross-workspace share is a
  404.
- Web: server load maps granted and unavailable; client page renders title,
  expiry, empty state, grid and masonry; unavailable page; Share control states
  (not shared, shared, copy success and failure, empty warning); keyboard
  operation and accessible names.

## Author's calls

Not discussed with the user; please correct any you disagree with.

- Token size is 128 bits (22 characters), stored plaintext.
- Expiry is checked with the database clock inside the lookup query.
- The empty-gallery warning is shown unconditionally for now, and gated on a
  real photo count when #66 introduces one.
- The "unavailable" copy and the empty-state copy above are proposals.
- The migration is new (`0003`) rather than an in-place edit.
- Layout rendering (grid and masonry) ships now and is verified with fixture
  photos, since #65 requires it and no real photos exist yet.
- The share route is `POST /galleries/{id}/share`, not a field patched through
  `PATCH`, because it is an action, not a user-editable setting.

## Left to the builder

- Exact names and module placement of `ShareToken`, `SharedGallery` and
  the shared `RandomToken` used by `SessionToken` and `ShareToken`.
- Masonry implementation technique (CSS columns or a script), as long as keyboard
  order and the viewer stay accessible.
- Exact copy wording, spacing and the browser-matrix choice for clipboard
  support.
- How logging is checked and redacted, given what the current server logs.
