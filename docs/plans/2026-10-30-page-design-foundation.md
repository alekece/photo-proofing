# Page Design Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the approved "Warm paper" look to the pages that already have a backend (sign in and first run, dashboard, gallery settings page, client page and 404), as issue #84.

**Architecture:** Design tokens live in `web/src/app.css` and feed the existing shadcn-style `Button` and `Input`. New small Svelte components (`AppHeader`, `PressAgainButton`, `NewGalleryDialog`, `GoldenHourEmpty`, `ViewfinderUnavailable`) sit in `web/src/lib/components/` and are tested with Testing Library, following `ShareLink.svelte` (injectable API functions). No backend change.

**Tech Stack:** SvelteKit 3, Svelte 5 runes, Tailwind 4, Vitest + Testing Library + vitest-axe, Playwright (`web/e2e`).

**Spec:** `docs/designs/2026-10-30-page-designs-design.md`. Mockups: now `docs/designs/ui/DESIGN.md` (the original `assets/pages/` mockups were replaced by #172). Issue: #84.

## Global Constraints

- Run from `web/`: tests `pnpm test <file>`, types `pnpm check`. Rust is untouched.
- Palette: ink `#2b2420`, page `#f4efe8`, panel `#fbf8f3`, borders `#e0d6c8` and `#d6c9b8`, danger `#b5381f`, info `#6b7f8e`.
- **Muted text is `#6f6256`** (5.16:1 on `#f4efe8`), not the mockup's `#7a6c5f` (4.44:1, fails AA).
- Georgia serif for headings and brand; system sans for UI; Lucide-style line icons (inline SVG; add no icon dependency).
- Press-again confirmation: second press within 5 seconds fires; the fill drains over those 5 seconds; Esc or click-away cancels. Honor `prefers-reduced-motion`.
- The client 404 must not reveal why a link is unavailable; keep the existing `Cache-Control: no-store` and `Referrer-Policy: no-referrer` on `/g/[token]`.
- New gallery dialog asks for a title only (max 255 characters, as `TITLE_MAX` in `routes/dashboard/[id]/+page.svelte`).
- Frontend rules from `AGENTS.md`: imports at the top, no unit-suffixed primitives where a type fits, document non-obvious contracts.

## Review Focus

- Press-again: a second press after the 5 s window must not fire; Esc and click-away disarm (Task 3).
- New gallery title: whitespace-only is rejected, 256 characters is rejected, a failed title save still opens the created gallery (Task 4).
- Dashboard search: regex characters match literally; an untitled gallery matches "untitled" (Task 5).
- Setup password confirmation: a mismatch never calls the API; the passwords are cleared after a failed request (Task 2).
- Client 404 text and markup contain no reason or token; reduced-motion users get no animation (Task 7).

---

### Task 1: Warm paper tokens and app shell

**Files:**
- Modify: `web/src/app.css`, `web/src/routes/+layout.svelte`
- Create: `web/src/lib/components/AppHeader.svelte`, `web/src/lib/components/AppHeader.test.ts`, `web/src/lib/theme.test.ts`

**Interfaces:**
- Produces: CSS variables `--background` (`#f4efe8`), `--foreground`, `--muted-foreground`, `--panel`, `--border`, `--danger`; `AppHeader` props `{ back?: { href: string; label: string }; onsignout?: () => void | Promise<void> }` rendering the brand "PhotoProofing" in Georgia, the optional back link and a Sign out button.

- [ ] **Step 1: Write `theme.test.ts`** that reads `app.css` with `fs`, extracts the custom properties, and asserts `contrast(--muted-foreground, --background) >= 4.5`, same for `--danger` and `--foreground` on `--background` and `--panel` (WCAG relative-luminance formula, written in the test).
- [ ] **Step 2: Write `AppHeader.test.ts`:** renders the brand; shows the back link only when `back` is given; clicking "Sign out" calls `onsignout`; no axe violations (`vitest-axe`, as in `routes.test.ts`).
- [ ] **Step 3: Run both; expect FAIL** (`pnpm test lib/theme.test.ts lib/components/AppHeader.test.ts`).
- [ ] **Step 4: Replace the tokens in `app.css`** with the palette above, re-point `.cn-button*`/`.cn-input` to them (default button ink on paper, outline button bordered, destructive uses `--danger`), and set Georgia on `h1, h2, .brand`. Drop the blue accent from `+layout.svelte` and use `--focus: #2b2420`.
- [ ] **Step 5: Create `AppHeader.svelte`** to the interface above.
- [ ] **Step 6: Run the tests; expect PASS**, then `pnpm check`.
- [ ] **Step 7: Commit** `feat: add warm paper tokens and app header`.

### Task 2: Sign in and first-run setup

**Files:**
- Modify: `web/src/routes/login/+page.svelte`, `web/e2e/auth.spec.ts`
- Test: `web/src/routes/login/page.test.ts` (create; there is none today)

**Interfaces:**
- Consumes: `authRequest('setup' | 'login', { email, password })` from `#lib/auth.js`.

- [ ] **Step 1: Write the failing tests** (mock `#lib/auth.js` and `$app/*` like `routes/dashboard/page.test.ts`): in setup mode a "Confirm password" field exists and a mismatch shows the role="alert" message "Passwords don't match." without calling `authRequest`; in sign-in mode there is no confirm field; after a rejected request both password fields are empty.
- [ ] **Step 2: Run; expect FAIL.**
- [ ] **Step 3: Implement** the confirm field (`autocomplete="new-password"`) in setup mode only, the mismatch check at the top of `submit`, clearing both passwords in the `catch` path, and the paper styling (centered card on `--background`, brand above the title).
- [ ] **Step 4: Update `e2e/auth.spec.ts`** to fill the confirm field in setup. Run `pnpm test routes/login`; expect PASS. Run `pnpm test:auth` if the e2e stack is available; otherwise report it was not run.
- [ ] **Step 5: Commit** `feat: confirm the password on first-run setup`.

### Task 3: PressAgainButton

**Files:**
- Create: `web/src/lib/components/PressAgainButton.svelte`, `web/src/lib/components/PressAgainButton.test.ts`

**Interfaces:**
- Produces: `PressAgainButton` props `{ label: string; confirmLabel: string; onconfirm: () => void | Promise<void>; disabled?: boolean; variant?: ButtonVariant }`. Exports `CONFIRM_WINDOW = 5000` from its `module` script. Used by Task 6 and later by #71 and #73.

- [ ] **Step 1: Write the tests** with `vi.useFakeTimers()`: first click shows `confirmLabel` and does not call `onconfirm`; second click within the window calls it once; advancing past `CONFIRM_WINDOW` restores `label` and a later click only re-arms; Escape and a click outside disarm; `disabled` blocks arming; the armed button exposes `aria-live="polite"` text.
- [ ] **Step 2: Run; expect FAIL.**
- [ ] **Step 3: Implement** with a `$state` armed flag, one timer, a document-level `keydown`/`pointerdown` listener only while armed, and a draining fill driven by a CSS animation of `CONFIRM_WINDOW` (disabled under `prefers-reduced-motion`).
- [ ] **Step 4: Run; expect PASS.** Commit `feat: add press-again confirmation button`.

### Task 4: New gallery dialog (title only)

**Files:**
- Create: `web/src/lib/components/NewGalleryDialog.svelte`, `web/src/lib/components/NewGalleryDialog.test.ts`

**Interfaces:**
- Consumes: `createGallery()` and `updateGallery(id, { title })` from `#lib/gallery/api.js`.
- Produces: `NewGalleryDialog` props `{ onclose: () => void; create?: typeof createGallery; update?: typeof updateGallery }`; on success calls `goto('/dashboard/<id>')`.

- [ ] **Step 1: Write the tests:** submitting "Martin wedding" calls `create` once then `update(id, { title: 'Martin wedding' })` then `goto`; whitespace-only and 256-character titles show an inline error and call nothing; when `update` rejects, `goto` still runs for the created gallery; a 401 goes to `/login?expired=1`; Esc and Cancel call `onclose`; focus starts in the title field; no axe violations.
- [ ] **Step 2: Run; expect FAIL.**
- [ ] **Step 3: Implement** with a native `<dialog>` opened via `showModal()` (gives focus trap and Esc), trimming the title before validation.
- [ ] **Step 4: Run; expect PASS.** Commit `feat: add title-only new gallery dialog`.

### Task 5: Dashboard look, search, empty states

**Files:**
- Modify: `web/src/routes/dashboard/+page.svelte`, `web/src/routes/dashboard/page.test.ts`
- Create: `web/src/lib/components/GoldenHourEmpty.svelte`

**Interfaces:**
- Consumes: `AppHeader` (Task 1), `NewGalleryDialog` (Task 4).
- Produces: `GoldenHourEmpty` (no props): the golden-hour joke from `dashboard.html`, with a "New gallery" prompt text only (the button stays in the header).

- [ ] **Step 1: Update and add tests** in `routes/dashboard/page.test.ts`: "New gallery" opens the dialog instead of creating immediately (adapt the existing create tests); typing in a "Search galleries" box filters cards by title, case-insensitively and literally (`a.b` does not match "axb"), and "untitled" matches untitled galleries; no match shows "No galleries match" and keeps the search box; zero galleries shows `GoldenHourEmpty` and no search box.
- [ ] **Step 2: Run; expect FAIL.**
- [ ] **Step 3: Implement** the paper layout from `dashboard.html` (cards, header via `AppHeader`, search field, existing sort select), the in-browser `$derived` filter, and `GoldenHourEmpty` as an inline SVG scene with a short fade (none under reduced motion).
- [ ] **Step 4: Run `pnpm test routes/dashboard`; expect PASS**, then `pnpm check`. Commit `feat: restyle the dashboard with search and golden-hour empty state`.

### Task 6: Gallery page layout and header action

**Files:**
- Modify: `web/src/routes/dashboard/[id]/+page.svelte`, `web/src/lib/components/ShareLink.svelte`, `web/src/routes/dashboard/[id]/page.test.ts`, `web/src/lib/components/ShareLink.test.ts`

**Interfaces:**
- Consumes: `PressAgainButton` (Task 3), `shareGallery` from `#lib/gallery/api.js`.

- [ ] **Step 1: Update the tests:** the header shows the title and "Draft · not shared" or "Shared"; before sharing, "Share gallery" is a press-again button that calls `shareGallery` only on the second press; after sharing, the link row (Client link input and Copy) replaces it and "Deliver to client" is shown disabled; the Photos / Favorites / Finals tabs exist with the Photos tab selected and its drop-zone text; existing autosave tests keep passing unchanged.
- [ ] **Step 2: Run; expect FAIL.**
- [ ] **Step 3: Restyle the page** to `gallery.html`: header block, settings panel on the left (layout choices, hero line, expiry, included photos, watermark toggle; keep all current autosave logic untouched), tabbed right pane with static empty states (upload and finals behavior arrive with #66 and #70; mark the buttons disabled with a title explaining why). Move the share action from `ShareLink` into the header via `PressAgainButton`, keeping the link and Copy row.
- [ ] **Step 4: Run `pnpm test routes/dashboard lib/components/ShareLink`; expect PASS**, then `pnpm check`. Commit `feat: restyle the gallery page with progress action`.

### Task 7: Client page and 404 viewfinder

**Files:**
- Modify: `web/src/lib/components/AccessUnavailable.svelte`, `web/src/lib/components/ClientGallery.svelte`, `web/src/routes/routes.test.ts`

**Interfaces:**
- Produces: `AccessUnavailable` keeps its name and no props; its content becomes the Canon-style viewfinder "Focus not locked" from `client-404.html`.

- [ ] **Step 1: Update tests** in `routes.test.ts`: `AccessUnavailable` shows an `h1` and the text "Please contact your photographer."; its text and markup contain none of expired, archived, deleted, shared or the word token; no axe violations; the existing neutral-page test still passes.
- [ ] **Step 2: Run; expect FAIL** on the new assertions.
- [ ] **Step 3: Implement** the viewfinder (inline SVG, a brief focus-hunt animation disabled under reduced motion) and give `ClientGallery` the paper background and Georgia title from `client-gallery.html`; keep the proofs behavior as is.
- [ ] **Step 4: Run `pnpm test routes/routes.test.ts`; expect PASS.** Commit `feat: restyle the client page and unavailable view`.

### Task 8: Record approved PRD changes

**Files:**
- Modify: `docs/SPECIFICATIONS.md`, `docs/releases/MVP.md`, `README.md` (only if its status or navigation text changes)

- [ ] **Step 1: Edit the PRD** for the six approved changes from the design's "PRD changes needing explicit approval": FR-04 (disabled Share replaces the empty-gallery warning); FR-10/AC-04 (no publish step, edits go live, an empty finals set after delivery is allowed); FR-11 and FR-12 (single "Mark all as read", clicking a notification marks its gallery read); the title-only New gallery dialog; no client "photos coming soon" page; photo ordering out of MVP.
- [ ] **Step 2: Align `docs/releases/MVP.md`** acceptance items that quote the old wording.
- [ ] **Step 3: Check** `grep -n "publish\|warning\|coming soon" docs/SPECIFICATIONS.md docs/releases/MVP.md` shows no stale wording. Commit `docs: record approved page design requirements`.

### Final check

- [ ] Run `pnpm test` and `pnpm check` in `web/`; report anything not run (for example `pnpm test:auth`). Then request an independent fresh-agent review of correctness and compliance with the design, as `AGENTS.md` requires.
