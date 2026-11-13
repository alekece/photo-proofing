# Client gallery slideshow playback

Issue: [#338](https://github.com/alekece/photo-proofing/issues/338). Touches `ClientGallery`, `PhotoViewer` (renamed from `ProofViewer`), `docs/SPECIFICATIONS.md`, and `docs/releases/MVP.md`.

## Intent

Clients reviewing proofs or published finals want an effortless way to sit back and watch their photographs advance sequentially from start to finish without clicking through every frame manually.

## Goals

- Rename `ProofViewer` to `PhotoViewer` (`PhotoViewer.svelte` / `PhotoViewer.test.ts`), since the same viewer renders both proofing photos and published finals across client and studio views.
- Offer a **Slideshow** / **Diaporama** action in the `ClientGallery` sticky bar whenever two or more photos are currently shown (`shown.length > 1`), in both proofing (`Prepublish`, respecting the active **All** / **Favorites** filter) and delivered (`Publish`) galleries, as well as in `ClientPreview`. Clicking it opens `PhotoViewer` at the first shown photo with playback running.
- Offer an icon-only Play / Pause control in the `PhotoViewer` top bar whenever `client` mode is active (`title !== undefined`) and `proofs.length > 1`, so clients can pause/resume a slideshow started from the sticky bar or start/stop one from any opened photo.
- Advance sequentially every **4 seconds** (`4000 ms`), with no speed-configuration UI.
- Pause automatically when the client manually navigates (`Previous`, `Next`, `ArrowLeft`, `ArrowRight`, `Home`, `End`), toggles a favorite, opens comments, reaches the last photo (`index === proofs.length - 1`), or closes the viewer.
- Clicking Play while already on the last photo restarts from photo 1 (`index = 0`) and plays through to the last photo.
- Accessible via keyboard and pointer/touch, localized in English and French, and respectful of `prefers-reduced-motion` (existing CSS instant transitions apply; timer still advances frames cleanly).

## Non-goals

- Slideshow controls in photographer studio tabs (`Photos`, `Favorites`, `Comments`, `Final`), where `title` is omitted so the viewer stays focused on curation.
- Speed controls, music, crossfade shaders, or fullscreen-API lock-in.
- Showing Play / Slideshow when `0` or `1` photo is shown.

## Constraints

- Clean up timers via `$effect` teardown inside `PhotoViewer` so closing the dialog, pausing, reaching the last frame, or unmounting never leaks intervals.
- At `390 px` mobile widths, keep both the `ClientGallery` sticky bar (where the **Slideshow** button sits alongside **All / ♥** or **Download all**) and the `PhotoViewer` top bar free of horizontal overflow in English and French (icon-only on narrow screens where needed, with localized `aria-label`).
- Update `docs/SPECIFICATIONS.md` and `docs/releases/MVP.md` (requires explicit maintainer approval of this design).

## Approaches

- **A. `PhotoViewer` owns playback state and timer; `ClientGallery` passes `autoplay` (chosen).** `ClientGallery`'s sticky-bar button sets `viewerProofs = shown`, `viewerIndex = 0`, `viewerAutoplay = true`. `PhotoViewer` owns `playing` and a single `$effect`-managed timer, so all auto-pause triggers (manual step, favorite, comments, reaching the last photo, unmount) stay in one place without cross-component timer coordination.
- **B. `ClientGallery` owns the timer and drives `index` externally.** Splits navigation/comment/favorite pause hooks across two components; rejected.

## Decisions

- Rename `web/src/lib/components/ProofViewer.svelte` → `PhotoViewer.svelte` and `ProofViewer.test.ts` → `PhotoViewer.test.ts`, updating imports in `ClientGallery`, `PhotosPanel`, `FavoritesPanel`, `CommentsPanel`, and `ViewerHarness.test.svelte`.
- Sticky-bar button in `ClientGallery.svelte`: shown when `shown.length > 1`; compact pill/outline button with `Play` icon + label (`Slideshow` / `Diaporama`, collapsing to icon-only with `aria-label` at `@max-[700px]` so `390 px` never overflows alongside the title and filter/download controls).
- Viewer button in `PhotoViewer.svelte`: shown when `client && proofs.length > 1`; icon-only `Play` / `Pause` button in the top bar with dynamic `aria-label` (`Play slideshow` / `Pause slideshow` — `Lire le diaporama` / `Mettre le diaporama en pause`).
- Timer behavior: when `playing` is true, if `index >= proofs.length - 1`, set `playing = false`; otherwise run a `4000 ms` timeout advancing `index += 1` (and when the step lands on `proofs.length - 1`, `playing` flips to `false` immediately so the Play icon shows on the final frame). Starting Play when `index === proofs.length - 1` sets `index = 0` and `playing = true`.
- Update `docs/SPECIFICATIONS.md` (`FR-04` / `FR-10`), `docs/releases/MVP.md`, and `docs/designs/ui/screens/client.md`.

## Left to the builder

- Exact button styling tokens in the sticky bar (`Button` outline/secondary vs pill matching the filter group) so it looks native next to both the proofing filter and the delivered `Download all` button.
- Fake-timer unit tests in `PhotoViewer.test.ts` and sticky-bar launch tests in `ClientGallery.test.ts` / `ClientPreview.test.ts`, plus `vitest-axe` checks in EN and FR.
