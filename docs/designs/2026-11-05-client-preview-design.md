# Client preview

Issue: #81.

## Goal

The photographer sees what the client will see, inside the app, without creating a link and whatever the gallery's access state.

## Behaviour

- A **Preview** button on the gallery page opens a full-screen overlay over the page. Esc or a Close button dismisses it.
- The overlay header shows a "Preview" marker, a Desktop and a Mobile icon toggle side by side (Desktop by default), and Close.
- **Desktop** renders the client page at full width. **Mobile** renders it in a centered phone-width frame (about 390px) with a fixed height that scrolls inside the overlay.
- The page is read-only: selection and download controls are visible but disabled. The photo viewer opens inside the preview.
- Always available to the signed-in owner, even when the gallery is unshared or expired. It never creates a share link and records no client activity. Other workspaces and signed-out visitors cannot reach it because there is no new route or endpoint: it only uses the owner's existing, authenticated gallery and photo calls.

## Design

- `ClientGallery` gets a `preview` flag. It adds the marker and disables write actions; later slices (#68 selection, delivery downloads) gate their controls on the same flag.
- A small mapper turns the owner's ready photos into the `Proof` list the client page takes, pointing at the owner's preview URLs (never the original). Same component and same data shape as the shared link, so the two cannot drift.
- The overlay is a Svelte component on the gallery page, fed from the gallery and photos the page already loaded. The frame width is a CSS container: the client grid sizes from its container width, so the mobile view needs no iframe.
- The overlay is a labelled modal: focus moves in, is trapped, and returns to the Preview button on close.

## Product documents

Preview is not in the PRD or MVP today. This change adds a short requirement to `docs/SPECIFICATIONS.md` (owner-only in-app preview, read-only, desktop and mobile views) and a line to the MVP scope and acceptance list in `docs/releases/MVP.md`. This design's approval is the product owner's approval of that scope change.

## Tests

- Preview shows for unshared and expired galleries and with no photos.
- Desktop and Mobile toggle changes the frame; Esc and Close dismiss and return focus.
- Selection and download controls are disabled and the "Preview" marker is present; the shared page without the flag is unchanged.
- Proofs use preview URLs only; no share link is created.
