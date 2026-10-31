# Frontend Design System Consolidation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing frontend use one theme, shared layout components and shared control primitives, with no visual or behavioural change beyond the Button `size` fix.

**Architecture:** All design values live in `web/src/app.css` `@theme`. Shared layout (`PageShell`, `Card`, `EmptyState`) and controls (`Label`, `Select`, `Checkbox`, `Tabs`, `Dialog`) live in `web/src/lib/components/ui/<name>/` following the Button/Input conventions (`tailwind-variants`, `cn()`, `data-slot`). Routes and feature components compose them. Delivered as two PRs.

**Tech Stack:** SvelteKit 3, Svelte 5 runes, Tailwind v4 (`@theme`), `tailwind-variants`, vitest + @testing-library/svelte + axe.

**Spec:** `docs/superpowers/specs/2026-10-31-frontend-design-system-design.md` (issue #89).

## Global Constraints

- Base: PR #88 (`feature/84-page-design-foundation`) holds the frontend being changed. Branch from `main` only after #88 merges; otherwise stack on that branch. Work in worktrees; branches `feature/89-theme-and-layout-foundation` (PR 1) and `feature/89-control-primitives-and-adoption` (PR 2).
- No visual redesign. Raw hex/rgb allowed only inside SVG artwork (`GoldenHourEmpty`, the SVG in `AccessUnavailable`).
- No new dependencies; Dialog/Select/Tabs/Checkbox are plain/native-element primitives. Keep native `<dialog>` and `<select>`.
- Primitives are added in the task that first uses them; no unused primitive.
- Preserve: focus return after dialog close, ARIA tabs with arrow keys, live regions, reduced motion, 2.75rem touch targets, global focus outline.
- Inline `style` only for runtime values, via `style:` directive or CSS variable.
- Checks (from `web/`): `npx vitest run`, `npm run check`, `npm run build`. Every PR records before/after screenshots of `/login`, `/dashboard`, `/dashboard/<id>`, and the client page `/g/<token>` (ready, empty, unavailable, viewer open) at 390px and 1280px.
- Rust/format rules in AGENTS.md don't apply (frontend only). Commit trailer: `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`.

## Review Focus

- Disabled Button: no hover darkening, still not focusable/clickable.
- `Button` `size="sm"` keeps a 2.75rem minimum touch target.
- `Dialog`: closing by Escape, Cancel, backdrop-less native `close`, and unmount all notify the parent exactly once and return focus to the opener.
- `Tabs`: Arrow/Home/End keys move focus and selection; only the active tab is in the tab order.
- `Select`/`Input`/`Checkbox`: `aria-invalid`/`aria-describedby` pass through to the element.
- Long titles with no spaces don't overflow the client page heading (`wrap-anywhere`).
- Disabled/`prefers-reduced-motion` behaviour unchanged.

---

# PR 1 — Foundation

### Task 1: Consolidate theme setup and tokens

**Files:**
- Modify: `web/src/app.css`, `web/src/routes/+layout.svelte`, `web/src/lib/components/ui/input/input.svelte`
- Modify (colours → tokens): `web/src/lib/components/ProofViewer.svelte`, `AccessUnavailable.svelte` (non-SVG CSS only), `ShareLink.svelte`, `Placeholder.svelte`, `PressAgainButton.svelte`, `NewGalleryDialog.svelte`, `GoldenHourEmpty.svelte` (CSS gradient only), `web/src/routes/dashboard/[id]/+page.svelte`, `web/src/routes/dashboard/+page.svelte`

**Interfaces:**
- Produces: Tailwind utilities from `@theme`: `font-sans`, `font-serif`, `bg-input`, `text-warning`, `bg-overlay`, `bg-viewer`, `text-viewer-foreground`, `bg-viewer-control`, `outline-viewer-focus`, `text-focus`/`outline-focus`, and radius tokens `rounded-card` (today's `rounded-lg`), `rounded-control` (`rounded-md`), `rounded-dialog` (`rounded-xl`); `rounded-full` stays for pills.

- [ ] **Step 1: Inventory colour usage.** Run `grep -rnE "#[0-9a-fA-F]{3,8}\b|rgb\(|stone-|amber-|bg-white|text-white" web/src --include=*.svelte --include=*.css` and write each distinct colour with its role in the PR description. Merge only identical or indistinguishable values.
- [ ] **Step 2: Add tokens to `@theme`** in `app.css`: `--font-sans` (system-ui stack now in `+layout.svelte`), `--font-serif` (Georgia stack, replaces `--serif`), `--color-focus` (`#2b2420`), `--color-input` (`#fff`), `--color-warning` (current `amber-800`), `--color-overlay` (`rgb(43 36 32 / 0.45)`), `--color-viewer*` (`#111`, `#fff`, `#222`, `#8ab4f8`), `--radius-card/control/dialog`. Point the `h1, h2, .brand` rule and global focus outline at the tokens.
- [ ] **Step 3: Remove the `<style>` block** (root font, `color-scheme`, `--focus`) from `+layout.svelte`, keeping `margin: 0; line-height: 1.5` on `body` in `app.css`.
- [ ] **Step 4: Replace colours and radii** in the listed files with token utilities / `var(--color-*)`; the Input `.cn-input` background uses `bg-input`. `stone-*` in `Placeholder` map to the nearest existing token or a new `--color-placeholder-*` token if it is artwork. Hard-coded SVG fills stay.
- [ ] **Step 5: Verify.** `grep` from Step 1 returns only SVG artwork lines; `npx vitest run` passes; `npm run check`; `npm run build`; screenshots show no difference.
- [ ] **Step 6: Commit** `refactor(web): move theme values into the Tailwind theme (#89)`.

### Task 2: Make Button `size` work

**Files:**
- Modify: `web/src/lib/components/ui/button/button.svelte`, `web/src/app.css`
- Test: `web/src/lib/components/ui/button/button.test.ts` (create)

**Interfaces:**
- Produces: `ButtonSize = 'default' | 'sm'` (drop unused `xs`, `lg`, `icon*`; only `sm` and default are used). `buttonVariants({ size })` returns real Tailwind classes, no `cn-button-size-*` names.

- [ ] **Step 1: Write failing tests** in `button.test.ts`: `buttonVariants({size:'sm'})` and `buttonVariants()` return different class strings, neither contains `cn-button-size-`; a disabled `<Button>` has `disabled` and the rendered class list includes the hover-exclusion (see Step 3).
- [ ] **Step 2: Run** `npx vitest run src/lib/components/ui/button/button.test.ts`; expect FAIL.
- [ ] **Step 3: Implement** sizes in the `tv` definition: `default` keeps today's padding; `sm` uses smaller horizontal padding and `text-sm` but keeps `min-h-11` (2.75rem). Change `.cn-button:hover` to `.cn-button:hover:not(:disabled)` (and `:not([aria-disabled=true])` for the link form).
- [ ] **Step 4: Run tests** expect PASS; `npm run check`.
- [ ] **Step 5: Commit** `fix(web): make Button size styles real (#89)`.

Note: `sm` previously rendered identically to default (no CSS existed); the 7 `size="sm"` buttons will visibly shrink horizontally. That is the intended fix; call it out in the PR with screenshots.

### Task 3: Drop inline style and dead fallback

**Files:** Modify `web/src/lib/components/ClientGallery.svelte:13`, `ProofThumbnail.svelte:28`

- [ ] **Step 1: Write failing test** in `ProofGrid.test.ts`/a new `ClientGallery.test.ts`: the heading has class `wrap-anywhere` and no `style` attribute for a 200-character unbroken title.
- [ ] **Step 2: Run** it; expect FAIL.
- [ ] **Step 3: Implement:** replace `style="overflow-wrap: anywhere"` with the `wrap-anywhere` utility; change `ProofThumbnail`'s outline to `var(--color-focus)` without the fallback.
- [ ] **Step 4: Run** tests; PASS. **Commit** `refactor(web): replace inline style and dead focus fallback (#89)`.

### Task 4: PageShell, Card and EmptyState, adopted

**Files:**
- Create: `web/src/lib/components/ui/page-shell/page-shell.svelte`, `ui/card/card.svelte`, `ui/empty-state/empty-state.svelte`, plus a `page-shell.test.ts` / `card.test.ts` / `empty-state.test.ts` each
- Modify: `routes/dashboard/+page.svelte:58,103,111`, `routes/dashboard/[id]/+page.svelte:220,291,422,432,446`, `routes/login/+page.svelte:52,65`

**Interfaces:**
- Produces: `<PageShell width?: 'narrow' | 'default' | 'wide'>` renders `<main class="mx-auto grid gap-6 p-6 …">` (`narrow` = `max-w-sm` login, `default` = `max-w-5xl`, `wide` = `max-w-6xl`; login keeps its `flex min-h-screen` via `class`). `<Card as?: 'div' | 'section' | 'li' | 'form'>` = `rounded-card border border-border bg-panel`. `<EmptyState>` = `rounded-card border border-dashed border-border-strong p-12 text-center`, centred children, `class` merges.
- Consumes: tokens from Task 1.

- [ ] **Step 1: Write failing tests:** each primitive renders its children, applies the documented classes, merges a caller `class`, and has no axe violations; PageShell renders a `main` landmark.
- [ ] **Step 2: Run** them; expect FAIL (modules missing).
- [ ] **Step 3: Implement** the three primitives following `button.svelte` conventions (`WithElementRef`, `cn()`, `data-slot`).
- [ ] **Step 4: Adopt** them in the listed routes, replacing the repeated class strings (3 empty tab panels become one `{#each}` over tab metadata using `EmptyState`).
- [ ] **Step 5: Run** `npx vitest run`, `npm run check`, `npm run build`; compare screenshots. **Commit** `feat(web): add PageShell, Card and EmptyState (#89)`.
- [ ] **Step 6: Open PR 1** referencing #89, assign @alekece, include the screenshots and the `sm` size note.

---

# PR 2 — Control primitives and adoption

### Task 5: Label and Input adoption

**Files:** Create `ui/label/label.svelte` + `label.test.ts`; modify `routes/login/+page.svelte`, `routes/dashboard/[id]/+page.svelte:226-377`, `NewGalleryDialog.svelte`.
**Interfaces:** Produces `<Label for?: string srOnly?: boolean>` rendering `<label class="cn-label …">`; the `.cn-label` CSS moves into the component's classes.

- [ ] **Step 1: Write failing tests:** `Label` renders a `<label>` associated by `for`, `srOnly` adds `sr-only`, no axe violations.
- [ ] **Step 2: Run;** FAIL. **Step 3: Implement** `Label`; replace all raw `<label>` (10) and replace the raw date/number/title `<input>`s in `[id]/+page.svelte` with `Input` (`type="date"`, `type="number"`, the inline title input through `class` override, keeping `aria-*`). Add an `Input` test that `type="date"` and `type="number"` pass through. **Step 4:** run suite; PASS; screenshots. **Step 5:** commit `feat(web): add Label and use Input everywhere (#89)`.

### Task 6: Select

**Files:** Create `ui/select/select.svelte` + `select.test.ts`; modify `routes/dashboard/+page.svelte:75-82`.
**Interfaces:** Produces `<Select bind:value class? …native select props>` wrapping a native `<select class="rounded-full border border-border-strong bg-panel px-3 py-1.5 text-sm">`.

- [ ] **Steps:** failing test (renders options, `bind:value` updates on change, `aria-*` passthrough, axe) → implement → adopt on the sort control → run `dashboard` page tests → commit `feat(web): add Select (#89)`.

### Task 7: Checkbox

**Files:** Create `ui/checkbox/checkbox.svelte` + test; modify `routes/dashboard/[id]/+page.svelte:299-310,374-377`.
**Interfaces:** Produces `<Checkbox bind:checked …native input props>` rendering a native `input[type=checkbox]` with shared classes.

- [ ] **Steps:** failing test (toggle via click, `bind:checked`, disabled, axe) → implement → adopt (the four checkboxes) → existing `[id]` page tests pass → commit.

### Task 8: Tabs

**Files:** Create `ui/tabs/tabs.svelte` (+ `tabs-list`, `tabs-panel` only if the API needs them) + `tabs.test.ts`; modify `routes/dashboard/[id]/+page.svelte:386-450`.
**Interfaces:** Produces `<Tabs bind:value items: { id: string; label: string }[]>` with a `panel` snippet receiving the active `id`; renders `role=tablist/tab/tabpanel`, `aria-selected`, `aria-controls`/`aria-labelledby`, roving `tabindex`, Arrow/Home/End key handling (behaviour moved from the page, not rewritten).

- [ ] **Steps:** failing tests (roles/ARIA wiring, ArrowRight/ArrowLeft/Home/End move focus and selection, inactive tab `tabindex=-1`, axe) → implement by moving existing logic → adopt → run `[id]/page.test.ts` unchanged → commit.

### Task 9: Dialog

**Files:** Create `ui/dialog/dialog.svelte` + `dialog.test.ts`; modify `NewGalleryDialog.svelte`, `ProofViewer.svelte:71-93`.
**Interfaces:** Produces `<Dialog onclose: () => void labelledby?: string label?: string class?>` wrapping a native `<dialog>`: opens with `showModal()` on mount (fallback `open` attribute), notifies `onclose` exactly once from the native `close` event (fallback path when `close()` is missing), Escape closes, restores focus to the opener, `::backdrop` styled from `--color-overlay` in the component's only `<style>`.
- Consumes: the close-handling logic from `NewGalleryDialog.svelte` (commit b44e830) and focus handling from `ProofViewer.svelte`.

- [ ] **Steps:** failing tests (opens modal; Escape calls `onclose` once; dispatching a native `close` event calls `onclose` once; focus returns to the opener on unmount; axe) → implement by lifting existing logic → adopt in both components, keeping their own content/buttons (`ProofViewer` buttons become `Button variant="ghost"`; add an icon-sized style via `class` only if needed) → existing `NewGalleryDialog` and `ProofViewer` tests unchanged and pass → commit.

### Task 10: Split the gallery detail page

**Files:** Create `web/src/lib/components/GallerySettings.svelte` (the settings `<aside>` at `[id]/+page.svelte:291-380`), `GalleryTabs.svelte` (tab list + panels); modify `routes/dashboard/[id]/+page.svelte`.
**Interfaces:** `GallerySettings` props: bindable draft values (`layout`, `watermark`, `expiry`, `included`, …) and handlers (`onsave`, `clearExpiry`), exactly what the page passes today; `GalleryTabs` props: counts/state used by the empty panels. The page keeps load data, save logic and header.

- [ ] **Steps:** run `[id]/page.test.ts` first (baseline PASS) → extract one component at a time, running the test after each → the page ends well under 250 lines → commit `refactor(web): split the gallery detail page (#89)`. The existing page tests are the regression net; add component tests only for new prop contracts.

### Task 11: Migrate client-side CSS to Tailwind

**Files:** Modify `ClientGallery.svelte`, `ProofGrid.svelte`, `ProofThumbnail.svelte`, `ExpiryBadge.svelte`, `AccessUnavailable.svelte`, `ProofViewer.svelte`, `PressAgainButton.svelte` as needed.

- [ ] **Step 1:** per component, convert layout/spacing/typography to utilities; keep `<style>` only for keyframes, pseudo-elements, `::backdrop`, `:focus-visible` rules Tailwind can't express, with a one-line reason comment.
- [ ] **Step 2:** run each component's existing test after converting it; they must pass unchanged.
- [ ] **Step 3:** confirm `grep -c "<style" web/src/lib/components/*.svelte` dropped and remaining blocks are justified.
- [ ] **Step 4:** full `npx vitest run`, `npm run check`, `npm run build`, screenshots at both widths; commit; open PR 2 referencing #89 with screenshots.

---

## Self-review

- **Spec coverage:** theme/tokens (T1), Button (T2), inline style/fallback (T3), layout primitives (T4), control primitives (T5–T9), page split (T10), client CSS (T11), screenshots (Global Constraints). Radius rule is in T1.
- **Consistency:** token names (`rounded-card/control/dialog`, `bg-input`, `text-warning`, `bg-viewer*`, `--color-focus`) are defined in T1 and consumed in later tasks.
- **Open for the executor:** exact prop API details inside the interfaces above, and mapping `Placeholder`'s `stone-*` shades after the inventory.
- **Spec tension flagged:** fixing Button `size` necessarily changes how 7 buttons look (they currently render at default size); the spec's "no visual change" holds for everything else.
