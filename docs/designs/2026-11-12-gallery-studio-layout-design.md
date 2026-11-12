# Studio gallery layout: Pixieset-style rail and top journey bar

Issue: [#342](https://github.com/alekece/photo-proofing/issues/342). Interactive Warm Paper mockups (desktop and `390 px` phone) were reviewed with the maintainer during brainstorming. Decisions marked *my call* are the author's defaults and open to change during review.

## Intent and why

Opening a gallery in the studio (`/dashboard/[id]`) currently renders a `400 px` dark client-style cover banner (`GalleryHeader` with an overlaid `AppHeader`) above a permanent `368 px` (`23rem`) left column of four stacked form cards (`GallerySettings`).

That layout creates three problems:

1. **Photos start too far down and too narrow.** The cover banner mimics the client cover rather than helping the photographer manage proofs, selections, comments, and finals, while the wide form-card column permanently consumes horizontal space next to the grid.
2. **Upcoming studio features will overflow the card stack.** Cover templates and focal point ([#345](https://github.com/alekece/photo-proofing/issues/345)), typography and grid presets ([#347](https://github.com/alekece/photo-proofing/issues/347)), gallery passwords ([#344](https://github.com/alekece/photo-proofing/issues/344)), contacts ([#339](https://github.com/alekece/photo-proofing/issues/339)), automatic expiry reminders ([#349](https://github.com/alekece/photo-proofing/issues/349)), and the gallery Activity view ([#350](https://github.com/alekece/photo-proofing/issues/350)) need dedicated workspace views rather than a tall scrolling sidebar of tiny cards.
3. **Photographers coming from Pixieset expect a calm left studio rail** separating collection views (`Photos`, `Favorites`, `Comments`, `Final`) from studio configuration (`Design`, `Settings`), while keeping PhotoProofing's 4-step Journey Wizard visible.

Client-facing gallery covers and `ClientPreview` are unaffected: this redesign applies strictly to the photographer's studio page (`/dashboard/[id]`).

## Goals

- Replace the overlaid banner header on `/dashboard/[id]` with the standard top `AppHeader` and a Pixieset-inspired **Warm Paper studio workspace**.
- Align user-facing gallery stage labels to **`Draft`** (private), **`Prepublish`** (proofing — favorites & comments), and **`Publish`** (finals ready to download), with `Expired` and `Archived` unchanged, and display the read-only status chip **inline to the right of the gallery title**.
- Keep the actionable 4-step **Journey Wizard** (`Upload → Share → Collect → Publish`) and primary stage actions (`Preview`, `Share`, `Publish`, `Restore`) prominent across the top of the main workspace.
- Organize the left studio rail into **Collection** (`Photos`, `Favorites`, `Comments`, `Final`) and **Studio** (`Design`, `Settings`), splitting `Settings` into **General**, **Privacy & Protection**, and **Actions**, and giving the active view the full remaining width of the stage.
- Adapt cleanly to `390 px` phones without hiding controls or pushing the photo grid below the fold (4–6 tiles visible immediately on open).
- Preserve all existing gallery invariants: inline title editing with debounce/flush/retry, mounted collection panels so counts and open viewers survive autosaves and tab switches, pre-share tab gating, notification read-on-open, keyboard accessibility, and English/French localization.

## Non-goals

- Adding new gallery capabilities belonging to follow-up issues (cover templates & focal point [#345](https://github.com/alekece/photo-proofing/issues/345), typography & grid density [#347](https://github.com/alekece/photo-proofing/issues/347), passwords [#344](https://github.com/alekece/photo-proofing/issues/344), contacts [#339](https://github.com/alekece/photo-proofing/issues/339), redesigned Share overlay [#348](https://github.com/alekece/photo-proofing/issues/348), reminders [#349](https://github.com/alekece/photo-proofing/issues/349), Activity tab [#350](https://github.com/alekece/photo-proofing/issues/350)). This issue establishes the studio layout those features will plug into.
- Changing any client-facing gallery page (`/g/[token]`), `ClientPreview` internals, or backend API endpoints.

## Anti-goals

- Duplicate form inputs, duplicate `id` attributes, or duplicate landmarks hidden via CSS across desktop and mobile breakpoints (which break `vitest-axe` and lose focus on resize).
- Unmounting `PhotosPanel`, `FavoritesPanel`, or `CommentsPanel` when switching to `Design` or `Settings` (which would lose loaded counts, active uploads/selections, or SSE state).
- Hiding `Archive`, `Restore`, or `Delete` in a desktop-only rail footer that disappears on `390 px` screens.

## Constraints

- Follow `docs/designs/ui/DESIGN.md` ("Warm paper" palette, Georgia headings, Lucide icons, pill buttons, `prefers-reduced-motion`, zero hint clutter, `390 px` mobile support).
- All user-visible strings (including `aria-label` and `title`) live in `web/messages/en.json` and `web/messages/fr.json`.
- Zero warnings across `pnpm --dir web verify`, including `vitest-axe` accessibility checks.

## Approaches considered

1. **Option A — Compact top header + full-width horizontal tabs, with `Design` and `Settings` as right-aligned tabs (rejected).** Gives 100% width to photos, but crowding 6–8 horizontal tabs (`Photos`, `Favorites`, `Comments`, `Final`, `Activity`, `Design`, `Settings`) onto one top tab bar mixes content and configuration and feels less familiar than Pixieset.
2. **Option B2 — Pixieset left rail + large editorial Georgia header + separate Journey Wizard card stacked on the main stage (rejected).** Looks airy on wide monitors, but on `390 px` phones the stacked title, metadata, full-width action buttons, Journey card, and section pills consume ~130 px more vertical height before the first photo appears.
3. **Option B1 — Pixieset left studio rail + top Journey Wizard bar on desktop, collapsing to a compact identity strip + 4-bar Journey row + swipeable pills on `390 px` (chosen).** Matches Pixieset's mental model on desktop, keeps PhotoProofing's signature 4-step Journey Wizard leading directly into the stage CTA (`Share` / `Deliver`), and puts 4–6 photo tiles above the fold on `390 px` phones.

## Decisions

### 1. Page shell and single-DOM responsive grid

- `/dashboard/[id]` renders standard `<AppHeader onsignout={logout} />` at the top (`border-b border-border`, non-overlaid). Remove the now-unused `overlay` prop from `AppHeader.svelte` (*my call*). Any sign-out error renders as an alert banner right below `AppHeader`.
- Below `AppHeader`, one responsive CSS Grid container arranges four semantic areas without duplicating DOM nodes across breakpoints (*my call*):
  - **Desktop (`>= 768px`, `md:`):** two columns (`16rem 1fr`, i.e. `256px` rail + fluid stage):
    - Left column (`bg-panel border-r border-border`): `identity` stacked above `nav` (sticky or full-height column).
    - Right column (`bg-background`, `min-w-0`): `journey` top bar (`bg-panel border-b border-border`) stacked above `stage`.
  - **Mobile (`< 768px`, down to `390 px`):** single column (`1fr`) ordered top-to-bottom:
    1. `identity` (`bg-panel`, compact horizontal row)
    2. `journey` (`bg-panel border-b border-border`, compact 4-bar progress + actions)
    3. `nav` (`bg-background border-b border-border`, horizontally scrollable pill strip)
    4. `stage` (`bg-background`, active section content)

### 2. Identity and cover card (`identity` area)

- **Back link:** `← Back to galleries` (`/dashboard`). Shows icon + label on `md:`, and a compact circular icon link (`aria-label={m.header_back_to_galleries()}`) on `< md` so the title fits on one row (*my call*).
- **Cover thumbnail:**
  - On `md:`, a `3:2` rounded card (`rounded-card overflow-hidden`) above the title.
  - On `< md`, a `44 × 34 px` rounded thumbnail beside the back button.
  - When `gallery.hero_id` is present, renders `<img src={previewUrl(gallery.id, gallery.hero_id, 'photos', heroRevision)} alt="" />` (preserving the same `<img>` element across title edits, dimmed/grayscale when `archived`).
  - When `gallery.hero_id` is `null`, renders `CoverBackdrop` with a `compact` mode (*my call*: scales initials proportionally to the card and omits the wide film-edge date line so small thumbnails stay crisp).
  - Clicking the cover thumbnail (`<button type="button" aria-label={m.studio_open_design()}>`) switches the active section to `Design` (*my call*).
- **Editable title + inline status chip:** Single `<Input id="title">` in Georgia serif (`font-serif`) paired **inline on its right** with the read-only Warm Paper `<StatusChip {status} dot />` (`Draft`, `Prepublish`, `Publish`, `Expired`, `Archived`), preserving `TITLE_MAX`, `600 ms` debounce, blur/beforeNavigate/pagehide `flush()`, and inline `"Not saved"` + `Retry` (`goingToRetry`) status right under the row.
  - *My call:* Keep the wire/database state identifiers (`'draft'`, `'shared'`, `'delivered'`, `'expired'`, `'archived'`) unchanged so no backend migration is needed, and update the English/French UI labels (`STATE_LABELS`, step 4, and the finals-publish button/dialog copy) to **`Draft` / `Prepublish` / `Publish`** (*Brouillon* / *Pré-publié* / *Publié*) across the studio and dashboard status filter.
- **Metadata facts:** Below the title row, show:
  - `Shot <date>` when `shoot_date` is set, otherwise `Created <date>` (`formatLongDate`, reading `shoot_date` at local noon `T12:00` so time zones never shift the calendar day).
  - Once shared: `Last opened <datetime>` (`formatDateTime`) or `Not opened yet` (`m.details_not_opened()`).
  - Omit the photo count here because both the `Photos` nav badge and Step 1 (`Upload`) already display it (*my call*, following `DESIGN.md` "no counts repeated in two places").

### 3. Top Journey Wizard and action bar (`journey` area)

- **Warm Paper `JourneySteps`:** Keep the 4 actionable steps (**`Upload` → `Share` → `Collect` → `Publish`** — guiding what to do rather than repeating status names) and their exact completion/fact/accent logic, restyled from dark photo-scrim tokens to Warm Paper surface tokens:
  - Completed step circle: solid `bg-ok border-ok text-inverse` with check icon; completed connector line: `bg-ok/55`.
  - Current step circle: `bg-golden-hour border-danger text-danger` (accent highlight); future step circle: `border-border-strong text-muted-foreground`.
  - Step label in `text-foreground` (muted on future steps); `step-fact` in `text-muted-foreground`, or `font-extrabold text-danger` when `accent` is true (selection over `included_photos`).
  - On `< sm` (`< 640px`), keep the compact 4-bar progress strip (`bg-ok`, `bg-danger`, `bg-border-strong`) + current step label and fact.
  - Hidden when `archived` (matching current behavior; the bar still holds `Preview` and `Restore`).
- **Stage actions (right side of the bar):**
  - `Preview` (`Button` `variant="outline"`, `size="sm"`, eye icon + label; icon-only with `aria-label={m.action_preview()}` below `sm:` if needed so `390 px` never wraps or clips — *my call*).
  - Primary stage button (`Button` default solid ink pill, `size="sm"`):
    - `Restore` when `archived`
    - `Share` when `!shared` (uses existing `PublishDialog` until [#348](https://github.com/alekece/photo-proofing/issues/348) introduces the `Share` studio view)
    - `Publish` when `shared && !delivered` (delivers finished photos via `PublishDialog`)
    - None once `delivered` (`Publish`) and not `archived` (until [#348](https://github.com/alekece/photo-proofing/issues/348) adds ongoing `Share` email invitations)

### 4. Studio navigation (`nav` area)

- Implemented as a single accessible `role="tablist"` (`aria-label={m.tabs_label()}`) supporting `ArrowDown`, `ArrowUp`, `ArrowRight`, `ArrowLeft`, `Home`, and `End` across enabled tabs (*my call*, so keyboard navigation feels natural both vertically on desktop and horizontally on mobile). Any visual group wrappers inside `role="tablist"` use `role="none"` and mark visual group headings (`Collection`, `Studio`) `aria-hidden="true"` so `role="tablist"` exposes only `role="tab"` children to assistive technology and `vitest-axe` (*my call*).
- Active section state lives in component state (`$state('photos')`, matching `GalleryTabs` today) without mutating the URL or re-running SvelteKit `load` functions (*my call*).
- Six items in two visual groups:
  - **Collection** (labelled heading on `md:`):
    1. `photos` — `Photos` + `photoCount` badge (always enabled; default active section on load)
    2. `favorites` — `Favorites` + `favoriteCount` badge (`accent` when over `included_photos`, `locked` padlock when `delivered`, `disabled` with `title={m.tab_after_sharing()}` when `!shared`)
    3. `comments` — `Comments` + `commentCount` badge (`disabled` with `title={m.tab_after_sharing()}` when `!shared`)
    4. `final` — `Final` + `finalCount` badge (`disabled` with `title={m.tab_after_sharing()}` when `!shared`)
  - **Studio** (labelled heading on `md:`; separated by a subtle divider on `< md` pill strip — *my call*):
    5. `design` — `Design` (always enabled, including in `Draft` and `Archived`)
    6. `settings` — `Settings` (always enabled, including in `Draft` and `Archived`)
- Active item uses solid ink fill (`bg-foreground text-background`) on both the desktop rail and the mobile horizontal pill strip, matching `DESIGN.md`'s selected control rule.
- If a gallery transitions state while open (for example `shared` becomes `true` after `Share`), `Favorites`, `Comments`, and `Final` enable immediately without reloading.

### 5. Stage panels (`stage` area)

- **Mounted collection panels:** `PhotosPanel` (proofs), `FavoritesPanel`, `CommentsPanel`, and `PhotosPanel` (`set="finals"`) remain mounted inside `hidden={active !== id}` wrappers so counts load on page open, ongoing uploads/exports continue, and autosaves replacing `saved` never remount panels or close an open `ProofViewer`.
- **`Design` panel (`GalleryDesignPanel.svelte`):**
  - Strictly owns client visual aesthetics (`max-w-xl` Warm Paper `Card` today):
    - `Layout` (`IconButtonGroup`: staggered columns / even rows)
  - Structured so [#345](https://github.com/alekece/photo-proofing/issues/345) and [#347](https://github.com/alekece/photo-proofing/issues/347) can extend it into the two-column controls + live client preview layout (`Desktop` / `Phone`) without restructuring the page.
- **`Settings` panel (`GallerySettingsPanel.svelte`):**
  - Displays a section heading (`Settings`) and three Warm Paper `Card` sections (`max-w-2xl`):
    1. **General:** `Shoot date` (`DatePicker`, clearable), `Included photos` (`Input type="number"`, debounced + validated on blur, disabled once published), and `Expiration date` (`DatePicker`, clearable, saves end-of-local-day; ready for `Expiration reminder` in [#349](https://github.com/alekece/photo-proofing/issues/349)).
    2. **Privacy & Protection:** `Direct link` (`ShareLink` when `shareToken` is present, or `"Not shared yet"` when `null`) and `Watermark proofs` (`Switch`; ready for `Require password` + `Gallery password` in [#344](https://github.com/alekece/photo-proofing/issues/344)).
    3. **Actions:** `Archive` (or `Restore` when archived) and destructive `Delete gallery`, each with a concise one-line description (*my call*) and existing confirmation dialogs.
  - Placing **Actions** as Card 3 at the bottom of `Settings` keeps destructive buttons off the navigation rail and identically reachable on desktop and `390 px` phones.
  - All inputs keep today's exact in-place save, error-toast, and revert-on-failure behavior.
- Retire `GalleryHeader.svelte`, `GallerySettings.svelte`, and `GalleryTabs.svelte` once replaced by the new studio components (`GalleryStudioRail.svelte` / `GalleryDesignPanel.svelte` / `GallerySettingsPanel.svelte`, or cohesive equivalents) and migrate their unit tests so no dead components remain (*my call*).

### 6. Documentation updates

- `docs/SPECIFICATIONS.md` and `docs/releases/MVP.md`: update gallery header wording to the studio cover card (`FR-03`) and align user-visible stage terminology (`Draft`, `Prepublish`, `Publish` — `FR-10`, `FR-11`).
- `docs/designs/ui/DESIGN.md` and `docs/designs/ui/screens/gallery.md`: update the Gallery screen description and status/step names (`Draft`, `Prepublish`, `Publish`; `Upload → Share → Collect → Publish`; `General`, `Privacy & Protection`, and `Actions` cards in `Settings`).

## Testing

- **Studio rail & navigation tests** (replacing `GalleryHeader.test.ts` and `GalleryTabs.test.ts`):
  - Cover image uses `heroRevision` and keeps the same `<img>` node across title edits; `CoverBackdrop` renders when `hero_id` is `null`; clicking the cover opens `Design`.
  - Details show `Shot` when `shoot_date` is set (falling back to `Created`), and show `Last opened` / `Not yet` only once shared.
  - `Favorites`, `Comments`, and `Final` are disabled before sharing (`Available once the gallery is shared`) while `Photos`, `Design`, and `Settings` remain enabled; sharing enables the gated tabs immediately; keyboard arrow/Home/End navigation cycles enabled tabs only.
  - Inline `StatusChip` (`Draft`, `Prepublish`, `Publish`, `Expired`, `Archived`), Journey Wizard steps (`Upload → Share → Collect → Publish`), counts, over-allowance accent, and stage buttons (`Share`, `Publish`, `Restore`, `Preview`) match every gallery state.
- **`Design` & `Settings` panel tests** (replacing `GallerySettings.test.ts`):
  - Draft `"Not shared yet"` vs shared `Copy` button; `Layout`, `Watermark`, `Open until`, `Shoot date`, and `Included photos` patch saves, validation, and revert-on-failure; `Archive`, `Restore`, and `Delete` callbacks; no success ring/toast.
- **Page & integration tests** (`web/src/routes/dashboard/[id]/page.test.ts`, `GalleryEditing.test.ts`):
  - Switching between `Photos`, `Design`, and `Settings` and autosaving gallery fields never remounts loaded photo/favorite/comment panels or closes an open `ProofViewer`.
  - Title inline edit, debounce, blur flush, failure message, and `Retry` button work as before.
  - Zero `vitest-axe` violations across the full gallery studio page (verifying no duplicate IDs or invalid ARIA tab roles).

## Left to the builder

- Exact component split and file names for the rail, journey bar, and panels (following PascalCase rules in `web/AGENTS.md`).
- Fine spacing, typography sizing, Divider hairlines, and horizontal scroll shadows on the `< md` pill strip within `docs/designs/ui/DESIGN.md` tokens.
