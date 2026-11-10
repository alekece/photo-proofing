# Frontend design system consolidation — design

Status: approved by the maintainer. Issue: #89 (follow-up to #84 / PR #88).

## Intent

A review of the frontend (branch `feature/84-page-design-foundation`) found that colours are tokenised but layout, controls and styling approach are not yet consistent: the Button `size` prop does nothing, two styling approaches coexist, colours and fonts bypass the theme, and several controls are hand-built per page. The maintainer is a backend expert and expects one place for shared styling, the same spacing/flex setup coming from shared classes or components, and generic primitives used everywhere.

Goal: make the existing UI follow those rules without changing its look or behaviour.

## Goals

- Every colour, font and radius comes from the Tailwind `@theme` in `app.css`; raw hex/rgb only inside SVG artwork.
- Theme setup (fonts, `color-scheme`, focus colour) lives in `app.css` only.
- Button `size` works.
- Every control that exists today has a shared primitive in `src/lib/components/ui/`, and routes/feature components use it instead of raw elements.
- Page width/padding and card/empty-state styling come from shared components, not per-page class strings.
- The gallery detail page is split by responsibility (settings panel, side nav, tabs).
- Tailwind utilities are used wherever they express the CSS; component `<style>` blocks remain only for what Tailwind cannot express (`::backdrop`, keyframes, complex pseudo-elements).
- Inline `style` is used only for runtime-computed values, via `style:` directives or CSS variables.

## Non-goals / anti-goals

- No visual redesign, no new screens, no behaviour change. Anything that looks different after the change is a bug.
- No new primitives that nothing uses (primitives are built on demand).
- No loss of the existing accessibility behaviour (focus return, ARIA tabs, live regions, reduced motion, 2.75rem touch targets).

## Constraints

- Frontend stack stays SvelteKit + Tailwind + `tailwind-variants`; follow the existing `ui/` primitive conventions (Button, Input).
- Existing web tests (99 at the time of review) keep passing; new primitives get focused tests and axe checks like current components.
- Product requirements are unchanged, so `docs/SPECIFICATIONS.md` needs no edit.

## Approaches considered

1. **One PR, sequenced commits.** Simple, but a very large diff to review.
2. **Two PRs: foundation, then adoption.** Reviewable and testable in pieces; the primitives' API is reviewed before pages use it.
3. **Primitives purely on demand with no up-front scope.** Less design, but risks inconsistent APIs.

**Chosen (by the maintainer): 2, with primitives created on demand.** Meaning: a primitive is added in the PR that first uses it. Primitives the existing code already needs but lacks are included in one of the two PRs; none is deferred to a later issue.

## Plan shape

**PR 1 — Foundation**
- Move font, `color-scheme` and focus colour from `+layout.svelte` into `app.css` `@theme` (`--font-sans`, `--font-serif`, `--color-focus`); make `font-serif` use the app font.
- Add missing colour tokens (`warning`, input background, overlay, viewer surface/focus) and replace Tailwind built-in colours (`stone-*`, `amber-*`, `bg-white`) and hardcoded hex values outside SVG artwork.
- Fix Button `size` (define the variants in the `tailwind-variants` definition or CSS) and exclude disabled buttons from the global hover darkening.
- Add a radius rule (token or documented usage) so cards, inputs and badges agree.
- Remove the dead `#1a73e8` fallback and the inline `style=` in `ClientGallery` (use a `style:` directive or a variable if it is genuinely runtime-computed).
- Add the layout primitives the pages need now: `PageShell` (width option), `Card`, `EmptyState`.

**PR 2 — Adoption**
- Add the control primitives the existing code lacks, each introduced where first used: Label, Select, Checkbox/Radio (if used), Tabs, Dialog (replacing the hand-written dialogs in `NewGalleryDialog` and `ProofViewer`), and date/number input support via Input.
- Replace raw `<input>`, `<select>`, `<label>`, `<button>` in routes and feature components.
- Split `dashboard/[id]/+page.svelte` into a settings panel, side nav and tabs component; the page keeps data loading and save logic. Collapse the three near-identical empty tab panels into one loop/component.
- Migrate the client-side components (`ClientGallery`, `ProofGrid`, `ProofViewer`, `AccessUnavailable`, `ExpiryBadge`, `ProofThumbnail`) from hand-written component CSS to Tailwind utilities where Tailwind can express it.

## Calls made by the author (please check)

- **"No visual change" outranks "agreement":** the radius scale is the minimal set matching today's values; differing radii are only unified where the difference is clearly accidental and imperceptible.
- **Token mapping:** inventory current colour usage first; collapse only identical or visually indistinguishable shades, never visibly different ones. Amber becomes a `warning` background/text/border set as needed; the dark proof viewer gets its own `viewer-*` tokens.
- **No extra layout primitives** (Stack, Row, Field) beyond `PageShell`, `Card` and `EmptyState`; repeated flex layouts stay as Tailwind utilities unless the same combination appears three or more times.
- **Side nav and tabs** are separate components; Tabs is the primitive, the side nav is a feature component. The three empty tab panels are one shared `EmptyState` instance driven by data.
- **No new dependency** (superseded by [ADR 0004](../adr/0004-styleable-component-libraries.md)): the project has no headless-UI library (only `tailwind-variants`, `clsx`, `tailwind-merge`), so Dialog, Select, Tabs and Checkbox are plain-element primitives with shared classes, and the existing hand-written focus return, ARIA tabs and keyboard behaviour is moved into them, not rewritten. Native `<dialog>` and `<select>` are kept.
- **Verification:** existing + new vitest/axe tests, plus a manual before/after screenshot comparison of each route and the client page at mobile and desktop widths, recorded in each PR. No new visual-regression tooling.
- The `ClientGallery` inline `style` is replaced by a `style:` directive or a CSS variable if it is runtime-computed, otherwise by utilities.

## Left to the builder

- Exact primitive prop APIs, following the Button/Input conventions.
- Exact grouping of PR 2 commits.
- Delivery follows `AGENTS.md`: a focused GitHub issue per PR (or one issue with two PRs), isolated worktree, plan under `docs/plans/`, fresh-agent review.
