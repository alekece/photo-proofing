# Maintainer Review Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix every item of #319: shoot date, styled calendar, delivered preview, notification read on open, lifecycle buttons, no success toasts, and the visual polish.

**Architecture:** One backend slice (shoot date column + gallery-scoped notification read), one new UI primitive (shadcn-svelte Calendar on Bits UI wrapped in a `DatePicker`), then focused frontend changes per screen. One PR.

**Tech Stack:** Rust (actix-web, sqlx, `time`), PostgreSQL, SvelteKit 5, Tailwind 4, Paraglide, Vitest + Testing Library, Bits UI, `@internationalized/date`.

**Spec:** [`docs/designs/2026-11-10-review-polish-design.md`](../designs/2026-11-10-review-polish-design.md)

## Global Constraints

- Shoot date is PostgreSQL `date`, nullable; the API wire format is `"YYYY-MM-DD"` or `null`. Never a timestamp.
- Shoot date is never part of the client `SharedGallery` payload.
- Every user-visible text is a Paraglide message in both `web/messages/en.json` and `fr.json`; run `pnpm --dir web paraglide` after editing messages. Dates use `#lib/gallery/format.js` helpers with `getLocale()`.
- UI components live in PascalCase folders under `web/src/lib/components/ui/` with `X.test.ts` next to them.
- Rust: no free functions outside tests; blank lines around control flow, after `let` groups, before `return`; no `mod.rs`.
- Commits: conventional (`feat:`, `fix:`, `docs:`), no `Co-authored-by` trailer.
- Rust checks: `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace`. Web checks: `pnpm --dir web verify`.
- Respect `prefers-reduced-motion` for every new animation (the global rule in `app.css` must cover it).

## Review Focus

1. Double click on Save while a slow save is in flight → exactly one PATCH (Task 8 test).
2. Shoot date chosen in French locale near midnight / in a UTC+ zone → stored day equals the picked day (Task 3 test: `DatePicker` emits the picked `YYYY-MM-DD` string with no `Date` conversion).
3. Delivered gallery whose cover photo is not a final → preview shows finals only and no broken cover (Task 7 test).
4. Clearing the shoot date in settings → `shoot_date: null` sent, card falls back to Created (Task 5 test).
5. Opening a gallery with no unread notification → no error, no extra refresh loop (Task 2 API test returns 204; Task 6 test).

---

### Task 1: Shoot date in the database and API

**Files:**
- Create: `crates/database/migrations/0028_gallery_shoot_date.sql`
- Modify: `crates/database/src/gallery.rs` (`Gallery`, `GalleryChanges`, `COLUMNS`, row mapping, `update_in`, `GalleryChanges::is_empty`)
- Test: `crates/api/src/routes/galleries.rs` (`mod tests`), `crates/database/src/gallery.rs` (`mod tests`) if serde tests live there

**Interfaces:**
- Produces: `Gallery::shoot_date: Option<time::Date>` serialized as `"shoot_date": "YYYY-MM-DD" | null`; `GalleryChanges::shoot_date: Option<Option<time::Date>>` (double option, `Some(None)` clears), deserializing only `"YYYY-MM-DD"`.

- [ ] **Step 1: Write failing API tests** in `galleries.rs` tests:
  - `shoot_date_round_trips_and_clears`: POST gallery → body has `"shoot_date": null`; PATCH `{"shoot_date":"2026-06-14"}` → 200 with `"shoot_date":"2026-06-14"`; GET shows the same; PATCH `{"shoot_date":null}` → `null`; PATCH `{"title":"x"}` leaves it unchanged.
  - Extend `invalid_changes_and_requests_are_rejected` with `{"shoot_date":"2026-06-14T10:00:00Z"}`, `{"shoot_date":"14/06/2026"}` and `{"shoot_date":"2026-02-30"}` → 400.
  - Assert the shared gallery JSON (existing shared route test helper) has no `shoot_date` key.
- [ ] **Step 2: Run** `cargo test -p pp-api shoot_date` → FAIL (unknown field / missing key).
- [ ] **Step 3: Migration** `ALTER TABLE galleries ADD COLUMN shoot_date date;` with a one-line comment (the day of the shoot, chosen by the photographer; NULL when not given).
- [ ] **Step 4: Model.** Add the field after `last_opened_at`, include `shoot_date` in `COLUMNS`, map it in the row decoder, bind it in `update_in` as `shoot_date = CASE WHEN $13 THEN $14 ELSE shoot_date END`, count it in `is_empty`. Serialize/deserialize with a `time::serde::format_description!` module for `[year]-[month]-[day]` (declared once in `gallery.rs`). Doc comment: "`None` means no shoot date was given."
- [ ] **Step 5: Run** `cargo test --workspace` → PASS, then fmt + clippy.
- [ ] **Step 6: Commit** `feat: store an optional gallery shoot date`

### Task 2: Mark a gallery's notification read

**Files:**
- Modify: `crates/database/src/notifications.rs` (repository), `crates/api/src/routes/notifications.rs` (route, module docs, tests)

**Interfaces:**
- Produces: `NotificationRepository::mark_gallery_read(&self, workspace_id: WorkspaceId, gallery_id: GalleryId) -> Result<(), DatabaseError>`; route `POST /api/v1/notifications/galleries/{gallery_id}/read` → `204 No Content`, requires session and `Origin` like the other POSTs.

- [ ] **Step 1: Write failing tests** in `notifications.rs` route tests:
  - `reading_a_gallery_marks_only_its_notification`: two galleries each with an unread notification (reuse existing fixtures); POST for gallery A → 204; list shows A read, B unread, unread total 1.
  - `reading_a_gallery_without_notifications_is_a_no_op`: → 204, list unchanged.
  - `reading_another_workspaces_gallery_changes_nothing`: other workspace's unread stays unread; response 204.
  - Origin/session required (add the path to the existing guard test).
- [ ] **Step 2: Run** `cargo test -p pp-api notifications` → FAIL (404 route).
- [ ] **Step 3: Implement** the method as one `UPDATE notifications SET read_at = now() WHERE workspace_id = $1 AND gallery_id = $2 AND read_at IS NULL`, and the `read_gallery` handler registered before `read_one` in `configure`. Add the route to the module `//!` list.
- [ ] **Step 4: Run** `cargo test --workspace` → PASS; fmt + clippy.
- [ ] **Step 5: Commit** `feat: mark a gallery's notification read`

### Task 3: Calendar and DatePicker primitives (+ ADR)

**Files:**
- Modify: `web/package.json`, lockfile (`pnpm --dir web add bits-ui @internationalized/date`)
- Create: `web/src/lib/components/ui/Calendar/Calendar.svelte` (+ any sub-parts copied from the shadcn-svelte v1 Calendar registry, kept inside this folder), `ui/Calendar/Calendar.test.ts`
- Create: `web/src/lib/components/ui/DatePicker/DatePicker.svelte`, `DatePicker.test.ts`, `DatePickerHarness.test.svelte` if needed
- Create: `docs/adr/0004-styleable-component-libraries.md`
- Modify: `docs/designs/2026-10-31-frontend-design-system-design.md` (one line under "No new dependency": superseded by ADR 0004)

**Interfaces:**
- Produces: `DatePicker` props: `{ id: string; value: string | null /* 'YYYY-MM-DD' */; min?: string; placeholder?: string; clearable?: boolean; onchange: (value: string | null) => void; class?: string; 'aria-invalid'?: boolean; 'aria-describedby'?: string }`. Trigger is a field-styled `<button>` showing the date via `formatShortDate`-style helper or `placeholder`; the calendar opens in `ui/Popover`. Conversion uses `parseDate`/`CalendarDate.toString()` only (no JS `Date`).

- [ ] **Step 1: Write failing tests** (`DatePicker.test.ts`):
  - `shows the placeholder when empty and the formatted day when set`.
  - `picking a day emits YYYY-MM-DD` (click trigger, click day 14 of June 2026 → `onchange('2026-06-14')`).
  - `days before min are disabled` (`min='2026-06-10'` → day 9 has `aria-disabled` / `data-disabled`).
  - `French locale starts weeks on Monday and names months in French` (set Paraglide locale `fr`; first weekday header is lundi's abbreviation; heading contains `juin`).
  - `clear button emits null` when `clearable`.
  - `has no axe violations` (vitest-axe, as other ui tests).
- [ ] **Step 2: Run** `pnpm --dir web test -- DatePicker` → FAIL.
- [ ] **Step 3: Add dependencies and copy the Calendar** from shadcn-svelte (Svelte 5 / Tailwind 4 version) into `ui/Calendar/`, restyled with app tokens (`bg-panel`, `text-foreground`, `rounded-control`, accent for selected, `muted-foreground` for outside days). Pass `locale={getLocale()}` and `weekStartsOn` from the locale (`fr` → 1, `en` → 0).
- [ ] **Step 4: Implement `DatePicker`** with the interface above.
- [ ] **Step 5: Write ADR 0004** (Context: hand-built primitives vs ADR 0001; Decision: prefer well-known libraries whose components we copy or fully style, starting with shadcn-svelte/Bits UI; Consequences: migration tracked in #320, `pnpm audit` covers new deps).
- [ ] **Step 6: Run** `pnpm --dir web verify` → PASS.
- [ ] **Step 7: Commit** `feat: add a styled calendar date picker`

### Task 4: Use the DatePicker for expiry and restore

**Files:**
- Modify: `web/src/lib/components/GallerySettings.svelte:163-171`, `RestoreExpiredDialog.svelte:75-82`
- Test: `GallerySettings.test.ts`, `RestoreExpiredDialog.test.ts`

**Interfaces:**
- Consumes: `DatePicker` (Task 3). Expiry keeps `endOfLocalDay(day)` / `toLocalDate(...)` conversions; restore keeps `min={today}`.

- [ ] **Step 1: Update tests** that typed into `input[type=date]` to pick through the DatePicker (open trigger by its label, click day); keep their existing assertions (PATCH body `expires_at`, restore error when empty).
- [ ] **Step 2: Run** those tests → FAIL.
- [ ] **Step 3: Replace** both native inputs with `DatePicker` (expiry `clearable`, restore not).
- [ ] **Step 4: Run** `pnpm --dir web test` → PASS; `grep -rn 'type="date"' web/src` → no matches.
- [ ] **Step 5: Commit** `feat: pick expiry dates with the app calendar`

### Task 5: Shoot date in the web app and the specification

**Files:**
- Modify: `web/src/lib/gallery/schemas.ts` (`gallerySchema.shoot_date: v.nullable(v.pipe(v.string(), v.isoDate()))`), `web/src/lib/gallery/api.ts` (`GalleryChanges.shoot_date?: string | null`)
- Modify: `NewGalleryDialog.svelte` (optional DatePicker under the title; included in the existing title PATCH), `GallerySettings.svelte` (clearable shoot date row, commits `{ shoot_date }`)
- Modify: `web/messages/en.json`, `fr.json` (labels: "Shoot date"/"Date de la séance", "Shot"/"Séance", placeholder "Optional"/"Facultatif")
- Modify: `docs/SPECIFICATIONS.md:44-46`
- Test: `NewGalleryDialog.test.ts`, `GallerySettings.test.ts`, `schemas.test.ts`

**Interfaces:**
- Produces: `Gallery.shoot_date: string | null` used by Task 6.

- [ ] **Step 1: Write failing tests:** dialog with title + picked date sends one PATCH `{ title, shoot_date: '2026-06-14' }`; title only sends no `shoot_date` key; settings clear sends `{ shoot_date: null }`; schema rejects `shoot_date: 'x'`.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** the schema/type, dialog field and settings row.
- [ ] **Step 4: Spec:** in Gallery configuration add "optional shoot date (a calendar day, shown only to the photographer)" and change "asks for a title only" to "asks for a title and an optional shoot date".
- [ ] **Step 5: Run** `pnpm --dir web verify` → PASS.
- [ ] **Step 6: Commit** `feat: ask for the shoot date when creating a gallery`

### Task 6: Gallery header details card and notification read on open

**Files:**
- Modify: `GalleryHeader.svelte:45-64,87,111`, `web/src/lib/gallery/notifications.svelte.ts`, `web/src/routes/dashboard/[id]/+page.svelte`, messages (remove `header_meta_empty`, `header_meta_created` if unused; add `details_photos`, `details_shot`, `details_created`, `details_last_opened`)
- Test: `GalleryHeader.test.ts`, `NotificationBell.test.ts` or a `notifications` unit test, `routes/dashboard/[id]/page.test.ts`

**Interfaces:**
- Consumes: `Gallery.shoot_date` (Task 5), `POST /api/v1/notifications/galleries/{id}/read` (Task 2).
- Produces: `NotificationCenter.markGalleryRead(galleryId: string): Promise<void>` (clears `unreadFor(galleryId)` locally first, then posts, then refreshes).

- [ ] **Step 1: Write failing tests:**
  - Header renders a `<dl>` card: terms Photos / Shot / Last opened with values `12` / formatted shoot date / formatted last open; with `shoot_date: null` the term is Created; with 0 photos value is `0` and no "no photos yet" text; unshared → no Last opened column.
  - `CoverBackdrop` edge text uses the long shoot date when set, else the created date.
  - `markGalleryRead` posts to the new path and the gallery's badge disappears before the response.
  - Gallery page calls `markGalleryRead(id)` once on mount and again when `id` changes; not on notification events.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** the card (headings row in `text-xs text-muted-foreground`, values below in `font-semibold`, in the page's panel style) replacing the `meta` paragraph, and the edge date. Implement `markGalleryRead` and call it from the gallery page `$effect` keyed on the id.
- [ ] **Step 4: Run** `pnpm --dir web verify` → PASS.
- [ ] **Step 5: Commit** `feat: show gallery details as a card and read notifications on open`

### Task 7: Delivered preview shows the finals

**Files:**
- Modify: `web/src/lib/components/ClientPreview.svelte:34-58`, `web/src/lib/gallery/proofs.ts` (`toOwnerProofs` gains `set: PhotoSet = 'photos'`)
- Test: `ClientPreview.test.ts`, `proofs.test.ts`

**Interfaces:**
- Produces: `toOwnerProofs(galleryId: string, photos: Photo[], set?: PhotoSet): Proof[]` building finals URLs with `thumbnailUrl/previewUrl(galleryId, id, set, revision)`.

- [ ] **Step 1: Write failing tests:** delivered gallery with 3 proofs and 2 finals → preview fetches `?finals=true` (via `listPhotos(id, fetch, 'finals')`), renders exactly 2 thumbnails whose URLs are finals routes, passes no favourites, and downloads stay inert (`preview` prop unchanged). Undelivered gallery → behaviour unchanged (existing tests). Cover photo absent from finals → no hero image rendered, no error.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement:** when `gallery.states.includes('delivered')` load finals with `listPhotos(id, fetch, 'finals')` and build proofs with `toOwnerProofs(id, finals, 'finals')`; skip `listClientPreviewPhotos` and favourites; pass `hero_id: null` when the hero is not among the finals.
- [ ] **Step 4: Run** `pnpm --dir web test` → PASS.
- [ ] **Step 5: Commit** `fix: preview a delivered gallery with its finals`

### Task 8: Lifecycle buttons keep their label still and ignore repeat clicks

**Files:**
- Modify: `web/src/lib/components/SaveButton.svelte`, `web/src/routes/settings/+page.svelte:164-191` (Cancel/Reverted and `save`/`cancel` handlers)
- Test: `SaveButton.test.ts`, `routes/settings/page.test.ts`

**Interfaces:**
- Produces: `SaveButton` props unchanged (`status`, `disabled`, `onclick`, `class`); while `status !== 'idle'` it renders `aria-disabled="true"`, not `disabled`, and does not call `onclick`.

- [ ] **Step 1: Write failing tests:**
  - Saving/saved: button is not `disabled`, has `aria-disabled="true"`, clicking does not call `onclick`.
  - Settings page: two quick clicks on Save while the PATCH is pending → exactly one PATCH; Cancel during Reverted does nothing.
  - Faces carry no `translate-y-*` classes (labels fade only).
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement:** faces cross-fade only (opacity), icon in a fixed-width slot so the label does not shift; keep `ClickRipple`; remove `disabled` during saving/saved/reverted and use `aria-disabled` + handler guards (`if (status !== 'idle') return`) in both component and page; `disabled` stays only for "nothing to save".
- [ ] **Step 4: Run** `pnpm --dir web test` → PASS; check visually in the browser (Settings Save and Cancel).
- [ ] **Step 5: Commit** `fix: keep lifecycle button labels still and ignore repeat clicks`

### Task 9: No success toasts; result and delete dialog copy

**Files:**
- Modify: `routes/dashboard/[id]/+page.svelte:197,209,232`, `routes/settings/+page.svelte:91-92`, `PublishDialog.svelte` (`resultBody`), `DeleteGalleryDialog.svelte` (`cancelLabel`), messages (remove `state_delivered`/`state_archived`/`gallery_restored`/`studio_regenerating`/`studio_quality_regenerating`/`delete_keep` if no longer used)
- Test: `routes/dashboard/[id]/page.test.ts`, `routes/settings/page.test.ts`, `PublishDialog.test.ts`, `GalleryDialogs.test.ts`

- [ ] **Step 1: Update/write failing tests:** deliver, archive, restore and settings save show no toast; failures still show an error toast; publish result has no link input and no Copy button for share and deliver; delete dialog's secondary button reads "Cancel" (`fr`: "Annuler").
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement** the removals; leave `toasts.success` in `toasts.svelte.ts` only if still used, otherwise delete it with its test.
- [ ] **Step 4: Run** `pnpm --dir web verify` and `grep -rn "toasts.success" web/src --include=*.svelte` → none.
- [ ] **Step 5: Commit** `feat: drop success toasts and the result link`

### Task 10: Photos tab polish

**Files:**
- Modify: `PhotoTile.svelte` (`GRID_CLASSES.masonry`, placeholder, image fade-in), `web/src/app.css` (shimmer keyframes/token), `ui/SelectActions/SelectActions.svelte`, `PhotosPanel.svelte:549-568`, `FavoritesPanel.svelte:255`
- Test: `PhotoTile.test.ts`, `ui/SelectActions/SelectActions.test.ts`, `PhotosPanel.test.ts`

- [ ] **Step 1: Reproduce masonry** in the browser (dev server, same gallery in grid then masonry at ≥1280 px). Record the cause in the commit message.
- [ ] **Step 2: Write failing tests:**
  - Placeholder tile has `animate-shimmer` (or the chosen token class); ready image gets `animate-photo-in` once `load` fires.
  - SelectActions renders "Select all" and "Unselect all" as `Button`s (no checkbox role) inside the right-aligned action group; clicking them calls `onselectall` / `onunselectall`; "Unselect all" is disabled with nothing selected.
- [ ] **Step 3: Run** → FAIL.
- [ ] **Step 4: Implement:** shimmer gradient over `--color-placeholder-tile` (new `--animate-shimmer`), image fade-in on load, masonry columns so tile width matches `minmax(11.25rem,1fr)` grid tiles, SelectActions restyled like the page's other toolbars (panel background, border tokens; no golden bar) with the two buttons on the right.
- [ ] **Step 5: Run** `pnpm --dir web verify` → PASS; check light/dark and reduced motion in the browser.
- [ ] **Step 6: Commit** `feat: polish photo tiles and the selection toolbar`

### Task 11: Caption icons and empty dashboard

**Files:**
- Modify: `PhotoTile.svelte:227-240` (caption icon), `EmptyWorkspace.svelte` (replace with `ui/EmptyState`), `routes/dashboard/+page.svelte` if it passes props
- Test: `PhotoTile.test.ts`, `EmptyWorkspace.test.ts`

- [ ] **Step 1: Write failing tests:** favourited caption icon has size 16 and `text-danger` (or heart token) class; downloaded caption icon size 16 and `text-ok-on-dark`; empty dashboard renders the EmptyState heading `m.dashboard_empty()` and the New gallery action, and no element with the old contact-sheet markup.
- [ ] **Step 2: Run** → FAIL.
- [ ] **Step 3: Implement**; delete the unused contact-sheet CSS and messages.
- [ ] **Step 4: Run** `pnpm --dir web verify` → PASS.
- [ ] **Step 5: Commit** `feat: clearer caption icons and a plain empty dashboard`

### Task 12: Final verification

- [ ] **Step 1:** `cargo +nightly fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings && cargo test --workspace` → PASS.
- [ ] **Step 2:** `pnpm --dir web verify && pnpm --dir web audit --prod` → PASS.
- [ ] **Step 3:** Walk the #319 checklist in the browser (light and dark, en and fr) and tick each item in the PR description.
