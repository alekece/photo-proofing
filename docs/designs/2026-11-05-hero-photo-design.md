# Hero photo

Issue: #85.

## Goal

The photographer marks one ready photo as the gallery's hero with a star. The dashboard card and the client gallery cover show that photo's preview. There is no automatic default: a gallery has one hero or none.

## Behaviour

- Each ready photo in the gallery's Photos tab has a star button at its top right. It shows on hover and keyboard focus, always on touch screens, and always on the hero.
- Starring another photo moves the hero to it. Starring the hero clears it. Each click saves at once; there is no Save step.
- Deleting the hero photo clears the hero. It does not fall back to another photo.
- No hero: the dashboard card shows its neutral placeholder and the client page shows no cover.
- The hero can change at any time, including after sharing and delivery.
- The settings panel's "Hero photo: set once photos are added" line is replaced by a hint to star a photo in the Photos tab.

## API and storage

- `galleries.hero_id uuid NULL`, a foreign key to `photos(id)` with `ON DELETE SET NULL`.
- `PATCH /api/v1/galleries/{id}` accepts `hero_id`, a photo id or `null` to clear it, with the same keep and clear semantics as the other optional settings. The update is refused (404 for an unknown photo, same as other missing resources) unless the photo is ready, in the same gallery and in the caller's workspace, so a photo from another gallery or workspace is never accepted.
- `Gallery` exposes `hero_id`. The dashboard card and the shared page build the preview URL from it with the existing preview route; the original is never used.
- The shared-gallery payload carries `hero_id` only if that photo is still ready.

## Web

- `gallerySchema` gains `hero_id` (nullable); the shared schema gains the same.
- `PhotoTile` gets the star toggle (`aria-pressed`, label "Set as hero photo" or "Remove hero photo"). The Photos panel calls the gallery update and refreshes the tile state; a failed save shows an error and leaves the previous hero.
- `GalleryCard` and the client cover render the hero's preview image.

## Tests

- Database: set, change, clear, reject a photo of another gallery or workspace, reject a non-ready photo, clear on photo deletion, change after share.
- API: PATCH round trip and the rejection cases above.
- Web: the star's states and accessible name, one click saves, a failed save keeps the old hero, the card and the cover use the preview URL and fall back to the placeholder.
