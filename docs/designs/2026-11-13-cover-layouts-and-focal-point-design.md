# Selectable client cover layouts and cover focal point

Issue: [#345](https://github.com/alekece/photo-proofing/issues/345). Builds on `GalleryDesignPanel` (#342) and touches `crates/database`, `crates/api`, `web/`, `docs/SPECIFICATIONS.md`, and `docs/releases/MVP.md`.

## Intent

Every client gallery currently uses one full-bleed cover crop fixed at `50% / 35%`, which awkwardly crops portraits and leaves photographers no editorial choice. Adding four curated cover templates, an interactive focal-point crosshair, an in-place cover picker, and an embedded live client preview in `Studio → Design` lets the photographer shape the client's first impression with immediate visual feedback.

## Goals

- **Studio `Design` two-column workspace (`GalleryDesignPanel`):**
  - **Left column (controls):**
    - **Cover card:** `Select cover` / `Change cover` button (opening a modal picker of the gallery's ready proof photos, plus `Remove cover` when one is set), **4 visual cover-template mini-cards** (`Full bleed`, `Split editorial`, `Framed paper`, `Minimal banner`), and an interactive **Focal point** rule-of-thirds pad (`x%, y%` draggable/clickable crosshair + keyboard nudging over the cover preview, disabled when no cover photo is set).
    - **Look card:** existing grid/masonry **Layout** toggle.
  - **Right pane (live preview):** embedded scrollable `ClientGallery` (`preview={true}`) with a `Desktop` / `Phone` switcher (plain `"Phone"` label, reusing the `ClientPreview` phone frame).
- **Four curated cover templates (shared inside `ClientGallery`):**
  1. `full_bleed` (default): full-screen hero with dark scrim, top studio signature, bottom title, optional shoot date, photo count, optional expiry badge, and CTA.
  2. `split`: side-by-side cover image and warm-paper (`#f4efe8`) typography panel on desktop; stacked cleanly on phones (`@max-[700px]`).
  3. `framed`: matted/framed photograph centered on warm paper with studio signature, title, metadata, and CTA arranged cleanly around/below it.
  4. `banner`: editorial signature, title, metadata, and CTA above a wide panoramic banner frame.
  - Every template renders title, studio signature, `shoot_date` (when set), photo count, expiry badge, and CTA (`View the gallery`, plus `Download all` when delivered), and falls back gracefully to `CoverBackdrop` initials when no cover photo is set.
- **Focal point everywhere:** `object-position: {x}% {y}%` (default `50%, 35%`) applies across the public client cover, `ClientPreview`, `Design` live preview, the studio left-rail cover thumbnail (`GalleryStudio`), and dashboard cards (`GalleryCard`), without modifying any stored image file.

## Non-goals

- Typography, grid density, and spacing presets (tracked separately in #347, which plugs into this `Design` workspace).
- Re-encoding or cropping image files on disk; focal point is pure CSS `object-position` metadata.

## Constraints

- **Rust types (`crates/AGENTS.md`):**
  - `CoverLayout` enum (`FullBleed` default, `Split`, `Framed`, `Banner`; `#[serde(rename_all = "snake_case")]`, SQL check/type `full_bleed | split | framed | banner`).
  - `FocalCoordinate` bounded integer percentage (`0..=100` validated at deserialization and SQL `CHECK (hero_focal_x BETWEEN 0 AND 100 AND hero_focal_y BETWEEN 0 AND 100)`) grouped in a `FocalPoint { x: FocalCoordinate, y: FocalCoordinate }` struct (default `x = 50`, `y = 35`), with no primitive unit suffixes and no free functions.
- Migration `0030_cover_layout_and_focal_point.sql` on `galleries` (`cover_layout`, `hero_focal_x`, `hero_focal_y`), with a transaction-scoped migration test in `crates/database`.
- Expose `cover_layout`, `focal_point` (`{ x, y }`), and `shoot_date` on both owner `Gallery` (and `GalleryOverview`) and client `SharedGallery` responses, and accept `cover_layout` and `focal_point` in `GalleryChanges` (`PATCH /api/v1/galleries/{id}`, debounced during focal-point dragging so pointer moves stay instant in the live preview without flooding `PATCH` requests).
- Localized in English and French (`web/messages/{en,fr}.json`), keyboard accessible, zero `vitest-axe` violations, and responsive at `390 px` (where the `Design` controls and live preview stack cleanly).

## Approaches

- **A. Embed the real `ClientGallery` (`preview={true}`) inside `GalleryDesignPanel` and share one set of cover templates (chosen).** `/g/[token]`, `ClientPreview`, and `Design` render the exact same markup and container queries (`@max-[700px]`), guaranteeing zero drift between preview and client view.
- **B. Build a separate scaled-down mock preview component for `Design`.** Duplicates all four cover layouts and drifts from the real client gallery; rejected.

## Decisions

- `FocalPoint` serializes as `{ "x": 50, "y": 35 }` on `Gallery`, `SharedGallery`, and `GalleryChanges` (stored in `galleries` columns `hero_focal_x smallint NOT NULL DEFAULT 50` and `hero_focal_y smallint NOT NULL DEFAULT 35`).
- `SharedGallery` now includes `shoot_date: Option<Date>` (`YYYY-MM-DD | null`); `docs/SPECIFICATIONS.md` (`FR-03`) is updated accordingly so client covers display the formatted shoot date when set.
- Focal-point pad supports pointer click/drag (`setPointerCapture`) and arrow-key steps (`1%`, `Shift+Arrow` `10%`), updating local reactive state immediately for the live preview and rail thumbnail while debouncing `PATCH` saves through the studio's existing `schedule`/`cancel` helpers.

## Left to the builder

- Visual polish of the 4 cover template mini-cards and cover-photo picker modal inside `GalleryDesignPanel`.
- Extracting a focused `ClientCover.svelte` subcomponent used by `ClientGallery.svelte` if `ClientGallery.svelte` grows too large with all 4 templates.
- Unit, integration, and migration tests across `crates/database`, `crates/api`, and `web/`.
