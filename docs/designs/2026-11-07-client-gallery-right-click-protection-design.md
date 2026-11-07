# Client gallery right-click protection

Status: approved by the maintainer. Issue: [#229](https://github.com/alekece/photo-proofing/issues/229).

## Intent

Discourage casual saving of photos from shared client galleries through the browser's image context menu, without making the gallery harder to use.

## Goals and non-goals

- Suppress the browser context menu when a visitor right-clicks an image in a client-facing shared gallery, including its cover, grid, and full-screen viewer. Suppress the iOS long-press image callout as the touch equivalent.
- Keep the normal context menu for non-image content and preserve the existing explicit download actions.
- Do not apply this to the photographer's dashboard or preview.
- Do not add a watermark or change download permissions. This is a deterrent, not a security guarantee.

## Approach

Use one handler scoped to the public gallery route. It prevents the default context menu when the event target is an image, a marked cover overlay, or a button containing a photo. A route-only style disables image callouts on iOS. This covers the gallery's image surfaces without suppressing the menu on other content or affecting the photographer's preview.

## Left to the builder

Follow the existing Svelte component patterns and add a focused regression test for image versus non-image context-menu behavior.
