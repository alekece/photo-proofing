# MVP page designs

Status: approved. Visual references live in
`docs/superpowers/specs/assets/pages/` (HTML is the source of truth, see its
README). Product requirements: `docs/SPECIFICATIONS.md` and
`docs/releases/MVP.md`. Issue: #84.

## Intent

Settle how every MVP page looks and behaves before building, so slices can be
implemented against one agreed reference instead of being designed ad hoc. The
pages are: first-run setup and sign in, dashboard, gallery, gallery preview,
client gallery, and client 404.

## Goals

- A warm, calm, photographer-friendly look ("Warm paper": cream and brown,
  Georgia serif headings, system sans UI, Lucide line icons).
- A gallery workflow that is obvious: upload, share, deliver, with the current
  step always visible in the header.
- Clients only ever see a gallery, never why a link is unavailable.

## Non-goals

- Backend design (schemas, APIs, upload, processing, export, SSE); each slice
  gets its own design.
- Brand name, logo and final copy. "PhotoProofing" is a placeholder.
- A Settings page (workspace watermark text and logo, account). Deferred; to be
  designed within the MVP.
- Failed-upload Retry, hero picker, watermark logo handling and photo ordering
  are not mocked.

## Anti-goals

- A client page that leaks whether a link was never shared, expired, archived or
  deleted (AC-01).
- Selection that the included-photo allowance can block.

## Decisions

- **Auth:** first run offers account creation with password confirmation; sign
  in shows a wrong-password error.
- **Dashboard:** gallery cards, search, filter (including Archived), sort,
  notifications panel with a single "Mark all as read". Empty state is the
  golden-hour joke; a separate no-results state exists. "+ New gallery" asks
  for a title only.
- **Gallery page:** settings panel (layout, hero, expiry, included photos,
  watermark toggle, Archive, Delete) and three tabs.
  - *Photos:* the rushes, with an upload drop zone when empty.
  - *Favorites:* only the client's selection, "n of N included", and a plain
    "Download" button (preparing, started, failed with retry).
  - *Finals:* same layout as Photos, with its own empty drop zone.
- **Header action:** a progress button, Share gallery, then Deliver to client,
  then "Delivered". Each step uses press-again confirmation (second press
  within 5 seconds, draining fill, Esc or click-away cancels). Delete uses the
  same pattern.
  - Share is disabled until a photo is ready; Deliver is disabled with no
    finals or while finals are uploading.
  - After delivery the gallery is live: finals changes reach the client
    silently. There is no publish step.
- **Delivered edits:** finals can be added or removed at any time. Removing
  every final leaves the client an empty delivered gallery (no revert, no 404).
- **Share is one-way:** a shared gallery cannot be un-shared. Archive is how
  access is cut (the client then sees the 404).
- **Hero photo:** defaults to the first photo; the photographer can select and
  change it at any time, including after sharing and delivery.
- **Preview:** the only way to see the client view from the gallery page. It
  opens the real client view in an overlay with a Desktop/Phone toggle;
  delivered galleries preview the delivered view with download arrows and
  Download all (ZIP). "Open as client" is removed.
- **Client gallery:** hearts, filter pill, lightbox; once delivered, per-photo
  download and Download all (ZIP).
- **Client 404:** the viewfinder "Focus not locked", shown for every
  unavailable link.
- **Full flow:** `prototype.html` is the reference for how pages connect.

## Approaches

- **A. Frontend first on mocks:** fast visuals, but risks rework when real
  behavior arrives.
- **B. Vertical slices:** each slice delivers a page with its backend, behind a
  shared shell built first; unbuilt backend features are stubbed. Chosen: it
  keeps the product shippable and each slice testable.
- **C. Design system first:** clean components, but delays visible progress.

## PRD changes needing explicit approval

1. FR-04: the empty-gallery share warning is replaced by a disabled Share
   button.
2. Delivery has no publish step; finals edits go live immediately, and an
   empty finals set after delivery is allowed (conflicts with FR-10/AC-04).
3. FR-11: the notification panel has a single "Mark all as read" link rather
   than a separate Mark as read control.
4. The New gallery dialog asks for a title only.
5. The client "photos coming soon" page is dropped, since a gallery cannot be
   shared without ready photos.
6. Photo ordering is out of the MVP unless specified.

## Calls made without the owner (check these)

- Failed uploads show a Retry on the tile (PRD FR-05); not mocked.
- The server re-checks that every final is ready when Deliver is pressed; the
  disabled button is not the only guard.
- Preview loads the client view through an authenticated photographer-only
  route, so it works before sharing; its hearts and downloads never create
  client activity or notifications in the real product (the prototype counts
  them only for demonstration).
- Favorites Download exports the client's selection as a ZIP of originals
  (FR-09), disabled when the selection is empty.
- The Settings page (workspace watermark, account) is a separate slice, ordered
  before watermark processing; until then the watermark toggle is hidden or
  inert.
- Notifications: clicking a gallery's notification marks that gallery's
  activity read; "Mark all as read" clears the rest.

## Left to the builder

Component structure in SvelteKit, Tailwind and shadcn-svelte; responsive
behavior beyond the Phone preview; accessibility details; motion timings other
than the 5-second press-again; copy refinements; offline and error states not
mocked, following the gallery mockup's simulations.
