# Client gallery typography, grid density, and spacing presets

Issue: [#347](https://github.com/alekece/photo-proofing/issues/347). Extends `Studio → Design` (#342, #345) and touches `crates/database`, `crates/api`, `web/`, `docs/SPECIFICATIONS.md`, and `docs/releases/MVP.md`.

## Intent

Photographers want client galleries to match the aesthetic pace of their work — airy editorial portraits versus denser event coverage — without complex layout builders. Adding four curated heading-typography presets alongside grid density and spacing controls in `Studio → Design` lets them tailor each gallery's visual rhythm with instant live preview.

## Goals

- **Heading Typography (4 curated presets in `Studio → Design`):**
  1. `editorial` (default): `Georgia`, warm serif.
  2. `fine_art`: `Cormorant Garamond`, literary high-contrast serif.
  3. `timeless`: `Playfair Display`, classic magazine display serif.
  4. `modern`: `Plus Jakarta Sans`, clean geometric sans.
  - Self-hosted via `@fontsource-variable` packages bundled by Vite so client pages never make external Google Fonts requests (preserving privacy and `Referrer-Policy: no-referrer`).
  - Applied strictly to the **client gallery cover heading and sticky-bar heading** via `--gallery-heading-font`; buttons, counts, badges, and studio UI controls remain in their standard font.
- **Photo Grid & Spacing (`Studio → Design → Photo Grid & Spacing`):**
  - **Layout:** `Staggered` (`masonry`) vs `Even rows` (`grid`).
  - **Grid density:** `Large` (fewer columns, larger photos — 1 column on `@max-[700px]` phones) vs `Compact` (default, matching today's grid — more columns, tighter contact-sheet feel, 2 columns on `@max-[700px]` phones).
  - **Spacing:** `Compact` (default, matching today's tighter gap) vs `Generous` (wider breathing room between photos).
- **Live Client Preview:** Every change saves in place and updates the embedded scrollable `ClientGallery` (`Desktop` / `Phone` frame using container queries `@max-[700px]`) and public client gallery immediately, while photographer management grids (`PhotosPanel`, `FavoritesPanel`, `CommentsPanel`) stay unchanged.

## Non-goals

- Custom font uploads, arbitrary pixel gap sliders, or applying gallery typography to photographer studio views.

## Constraints

- **Database schema (`0032_gallery_typography_and_grid_presets.sql`):**
  - Closed PostgreSQL enums on `galleries`:
    - `heading_font` (`editorial` default, `fine_art`, `timeless`, `modern`)
    - `grid_density` (`compact` default, `large`)
    - `grid_spacing` (`compact` default, `generous`)
  - Defaulting existing rows to `editorial`, `compact`, `compact` preserves the exact appearance of existing galleries.
- **Rust rules (`crates/AGENTS.md`):**
  - Validated `#[derive(..., sqlx::Type)]` enums `HeadingFont`, `GridDensity`, and `GridSpacing` (`#[serde(rename_all = "snake_case")]`) exposed on both owner `Gallery` and client `SharedGallery`, and accepted via `deserialize_present` in `GalleryChanges`. Zero free functions outside `#[cfg(test)]`; include a transaction-scoped migration test in `crates/database`.
- **Frontend (`web/AGENTS.md`):**
  - Install `@fontsource-variable/cormorant-garamond`, `@fontsource-variable/playfair-display`, and `@fontsource-variable/plus-jakarta-sans` in `web/package.json` (verify `pnpm --dir web audit --prod` passes cleanly).
  - Use container queries (`@max-[700px]`, `@min-[1400px]`) in `ProofGrid.svelte` so the `Phone` preview frame inside `Design` and `ClientPreview` matches real phones.
  - Localized in English and French (`web/messages/{en,fr}.json`), keyboard accessible, zero `vitest-axe` violations, and responsive at `390 px`.

## Approaches

- **A. CSS custom property `--gallery-heading-font` on `ClientGallery` + container-query preset map in `ProofGrid` (chosen).** Cleanly scopes heading typography to `ClientCover` and the `ClientGallery` sticky-bar `<h2>` without touching global `h1`/`h2` rules, and keeps `Desktop`/`Phone` preview identical to `/g/{token}`.
- **B. Per-element inline font-family branching across every cover template.** Repetitive and error-prone; rejected.

## Decisions

- `HeadingFont` values: `editorial | fine_art | timeless | modern` (default `editorial`).
- `GridDensity` values: `compact | large` (default `compact`).
- `GridSpacing` values: `compact | generous` (default `compact`).
- Update `docs/SPECIFICATIONS.md` (`FR-03`), `docs/releases/MVP.md`, `docs/designs/ui/screens/gallery.md`, and `docs/designs/ui/screens/client.md`.

## Left to the builder

- Exact column min-widths / counts and gap values for `large` vs `compact` density and `compact` vs `generous` spacing so each preset feels distinct and balanced on desktop and `340 px`/`390 px` phones.
- Visual styling of the 4 typography specimen buttons and segmented density/spacing controls in `GalleryDesignPanel.svelte`.
