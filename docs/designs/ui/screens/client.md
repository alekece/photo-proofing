# Client gallery

What the client sees at the private link `/g/{token}`. No account, no branding beyond the
studio's signature.

Right-clicking images in the cover, grid or viewer does not open the browser context menu. On iOS,
long-pressing an image does not show its image callout. These are casual deterrents, not security
features.

| | Desktop | Phone |
|---|---|---|
| Cover | ![](client/cover.desktop.png) | ![](client/cover.phone.png) |
| Cover, no hero | ![](client/cover-no-hero.desktop.png) | ![](client/cover-no-hero.phone.png) |
| Grid | ![](client/grid.desktop.png) | ![](client/grid.phone.png) |
| Favorites filter | ![](client/grid-favorites.desktop.png) | ![](client/grid-favorites.phone.png) |
| Viewer | ![](client/viewer.desktop.png) | ![](client/viewer.phone.png) |
| Delivered | ![](client/delivered.desktop.png) | ![](client/delivered.phone.png) |
| Delivered viewer | ![](client/delivered-viewer.desktop.png) | ![](client/delivered-viewer.phone.png) |
| Cover in French | ![](client/cover-fr.desktop.png) | ![](client/cover-fr.phone.png) |
| Grid in French | ![](client/grid-fr.desktop.png) | ![](client/grid-fr.phone.png) |

## Cover

Full-screen hero photo. The studio signature (name, logo, both or none, from
[Settings](settings.md)) sits at the top. At the bottom, centred: the title in Georgia, the photo
count and **View the gallery**, which scrolls to the grid. Once delivered, "Your photos are ready"
above the title and **Download all** beside the button.

When Both is selected, the centered logo sits above the centered name. Only the chosen public
signature fields are exposed; fetching the normalized logo requires an available private link,
just like the gallery and its photos.

**No hero:** the same warm dark gradient as the photographer's no-hero header, with the title and
details on it.

## Grid

- **Sticky bar:** title on the left; on the right a **Slideshow** button (when two or more photos
  are shown, opening the viewer at the first shown photo with playback active) beside an **All** /
  **♥ 13** toggle that filters to favorites (the number bumps when it changes). Once delivered,
  **Slideshow** sits beside **Download all** instead.
- **Photos** in the gallery's layout (staggered columns or even rows), watermarked when the
  gallery has watermarking on.
- **Heart** at the bottom right of each photo: shown on hover and focus, always on touch screens,
  always when favorited. One click saves; it pops on favorite. No submit button, no confirmation.
- Favorites filter with none: "No favorites yet".
- Proof tiles show a small badge with the nondeleted comment count when nonzero.
- **Delivered:** the grid shows the finished photos without watermark, each with a download
  button instead of a heart. Favorites are gone from view.
- Footer: "STUDIO NAME · PHOTOPROOFING".

![Heart motion](client/heart-motion.gif)
![Heart frames](client/heart-motion.frames.png)

## Viewer

Dark full screen, like the photographer's [viewer](viewer.md).

- **Top bar:** title, "4 of 48", Play / Pause slideshow (advancing every 4 seconds until the last
  photo, pausing on manual navigation, favorite toggles or opening comments), then on desktop the
  **Add to favorites** / **Favorite** button, and ✕.
- **Phone:** the heart button sits under the photo instead of the top bar, easy to reach with a
  thumb.
- **Delivered:** a **Download** button instead of the heart.
- **Comments:** the top-bar button shows the count and toggles a thread, closed by default.
  It sits beside the photo on desktop and below it on phones, never covering the image.
  Escape closes the thread before the viewer; changing photos closes it too. Clients write
  plain text (1–1000 characters), edit client messages, or delete them after confirmation.
  Deleted messages show a placeholder. Replies appear on reopening the thread or reloading,
  without email or live notifications. Delivered galleries and read-only Preview omit comments.
  Short landscape phones use the side panel instead, temporarily hiding the bottom favorite action
  while comments are open to keep the image and composer usable.
