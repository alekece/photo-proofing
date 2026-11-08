# English and French Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The photographer app, every client page and the unavailable page are available in English and French, chosen once per workspace with a switch beside the notifications bell.

**Architecture:** The workspace language is a new `workspace_settings.language` column, saved by its own endpoint and readable by anyone through a public installation endpoint. The web uses Paraglide JS: a custom server strategy reads the installation language for every request, the server writes it into `<html lang>`, and a custom client strategy reads it back after hydration. Every user-visible string becomes a typed message function backed by `web/messages/en.json` and `web/messages/fr.json`.

**Tech Stack:** Rust (actix-web, sqlx, Postgres), SvelteKit 3 with Svelte 5, `@inlang/paraglide-js` 3.2.1, Vitest.

**Spec:** [`docs/designs/2026-11-06-ui-languages-design.md`](../designs/2026-11-06-ui-languages-design.md), issue [#187](https://github.com/alekece/photo-proofing/issues/187), PRD [FR-02a](../SPECIFICATIONS.md).

## Global Constraints

- Exactly two locales, `en` (base) and `fr`. New workspaces start in `en`.
- One language per workspace (one per installation in the MVP), applied to the photographer app, every client page and the unavailable page.
- Before setup, when no workspace exists, the sign-in page follows the browser's `Accept-Language` (English or French), else English.
- Never translate the photographer's own content: gallery titles, studio name, file names.
- Dates follow the language: "12 Sep" in English and "12 sept." in French, formatted with `Intl` and the active locale, never a hard-coded `'en'` or `undefined`.
- The switch is a button showing "EN" or "FR" next to the notifications bell. Its menu offers "English" and "Français". It also stays in the top bar on phones.
- The web never shows the API's `error` text: it maps the HTTP status and field to translated messages.
- Layouts must hold the longer French text without clipping.
- Rust: no free functions outside tests. Group lines with blank lines (before control flow, after `let` groups, before return expressions).

## Review Focus

1. **Missing French string.** A key present in `en.json` but absent from `fr.json` would silently show English, which is the "mix of languages" anti-goal. Task 2 adds a parity test.
2. **The API is down when a page renders.** The language lookup must not turn every page into a 503. Fall back to `Accept-Language`, then English. Task 2 tests this.
3. **Hydration mismatch.** If the browser picks a different locale from the server (for example from `navigator.language`), the text flips after load. The client strategy must read `<html lang>` first. Task 2 tests this.
4. **An expired or unknown client link.** The unavailable page must be in the workspace language and must not reveal which gallery the link was for. Task 3 tests this.
5. **French text overflowing.** Long French labels such as "Livrer les photos finales" in buttons, pills and cards on a 375 px phone. Task 7 checks this manually against the prototype screenshots.

---

### Task 1: Workspace language in the API

**Files:**
- Create: `crates/database/migrations/0022_workspace_language.sql`
- Modify: `crates/database/src/watermark.rs` (settings repository)
- Modify: `crates/api/src/routes/settings.rs`, plus a new public route in `crates/api/src/routes/` registered in `crates/api/src/api.rs`
- Test: in-file `#[cfg(test)]` modules of the files above

**Interfaces:**
- Produces:
  - `GET /api/v1/language` is public and returns `200 {"language": "en" | "fr" | null}`. The value is `null` while setup is still open (no workspace). It sends `Cache-Control: no-store`.
  - `PUT /api/v1/settings/language` is authenticated, takes `{"language": "en" | "fr"}` (unknown fields are refused) and returns `200 {"language": ...}`.
  - `pub enum Language { En, Fr }` is an `sqlx::Type` and serde enum, with `type_name = "ui_language"` and snake_case values `"en"` and `"fr"`.
  - `WatermarkRepository::language(&self, WorkspaceId) -> Result<Language, DatabaseError>`
  - `WatermarkRepository::save_language(&self, WorkspaceId, Language) -> Result<(), DatabaseError>`
  - `WatermarkRepository::installation_language(&self) -> Result<Option<Language>, DatabaseError>`

- [ ] **Step 1: Write failing tests**
  - Repository tests:
    - `language_defaults_to_english_without_a_settings_row`
    - `saving_the_language_keeps_the_watermark_revision`: save `Fr`, then read it back, and `watermark_revision` is unchanged.
    - `installation_language_is_none_before_setup_then_the_workspace_language`
  - Route tests:
    - `language_is_public_and_null_before_setup`
    - `language_follows_the_saved_setting`
    - `saving_the_language_requires_a_session`: 401 without a cookie.
    - `saving_an_unknown_language_is_refused`: `"de"` returns 422.
- [ ] **Step 2: Run** `cargo test -p pp-database language` and `cargo test -p pp-api language`. Expect them to FAIL: the column, methods and routes do not exist yet.
- [ ] **Step 3: Implement**
  - The migration runs `CREATE TYPE ui_language AS ENUM ('en', 'fr')` and `ALTER TABLE workspace_settings ADD COLUMN language ui_language NOT NULL DEFAULT 'en'`.
  - `save_language` upserts with `INSERT … ON CONFLICT (workspace_id) DO UPDATE SET language = EXCLUDED.language, updated_at = now()`. It must not touch `watermark_revision`.
  - `installation_language` reads the only workspace, with the settings row left-joined and defaulted to `'en'`. It returns `None` when there is no workspace.
  - Keep `SettingsView` and `SettingsRequest` unchanged, so saving the watermark never resets the language.
- [ ] **Step 4: Run** the same tests. Expect PASS. Then run `cargo +nightly fmt --all` and `cargo clippy --workspace --all-targets --locked -- -D warnings`. Expect them to be clean.
- [ ] **Step 5: Commit** with `feat(api): save the workspace language`.

### Task 2: Paraglide wiring, locale strategies and dates

**Files:**
- Modify: `web/package.json` (add `@inlang/paraglide-js@3.2.1` as a dev dependency, and a `paraglide` compile script run by `prepare`, `check`, `test` and `build`), `web/vite.config.ts`, `web/src/app.html` (`lang="%paraglide.lang%"`), `web/.gitignore`, `web/.prettierignore` and `web/eslint.config.js` (ignore `src/lib/paraglide`)
- Create: `web/project.inlang/settings.json`, `web/messages/en.json`, `web/messages/fr.json`, `web/src/hooks.server.ts`, `web/src/hooks.client.ts`, `web/src/lib/i18n/strategy.ts`, `web/src/lib/i18n/dates.ts`, and `web/src/lib/i18n/language.ts` (the client `saveLanguage`)
- Modify: `web/src/lib/gallery/format.ts`, `GalleryHeader.svelte` and `ExpiryBadge.svelte` (dates)
- Test: `web/src/lib/i18n/messages.test.ts`, `web/src/lib/i18n/strategy.test.ts`, `web/src/lib/i18n/dates.test.ts`, `web/src/lib/i18n/language.test.ts`

**Interfaces:**
- Consumes: the Task 1 endpoints.
- Produces:
  - The generated `#lib/paraglide/messages.js` (`m`) and `#lib/paraglide/runtime.js` (`getLocale`, `setLocale`, `locales`). The output directory is `web/src/lib/paraglide`; it is not committed and is regenerated by the compile script.
  - The compiler strategy is `['custom-workspace', 'preferredLanguage', 'baseLocale']`.
  - `registerServerLocale(fetcher: typeof fetch): void` defines the `custom-workspace` server strategy (an async `GET /api/v1/language` through `apiUrl`). It returns `undefined` on `null`, a network failure or a bad body, so the next strategy decides.
  - `registerClientLocale(): void` defines the client strategy: `getLocale` returns `document.documentElement.lang`, and `setLocale` does nothing (the API holds the setting).
  - `formatShortDate(date: Date): string` gives "12 Sep" or "12 sept.". `formatLongDate(date: Date, timeZone?: string): string` gives dateStyle `long`. `formatDayMonthYear(date: Date): string` covers today's `format.ts:16`. All of them use `getLocale()`.
  - `saveLanguage(language: Locale, fetcher?: typeof fetch): Promise<void>` sends `PUT /api/v1/settings/language` and throws on anything other than 2xx.

- [ ] **Step 1: Write the failing tests**
  - `messages.test.ts`:
    - `en.json` and `fr.json` have the same keys.
    - No `fr` value is empty.
    - Every key whose `fr` value equals its `en` value is listed in `SAME_IN_BOTH` (start it empty; add words like "Photos" only deliberately).
  - `strategy.test.ts`:
    - The server strategy returns `'fr'` when the API answers `{"language":"fr"}`.
    - It returns `undefined` on `null`, on a fetch rejection and on a 503.
    - The client strategy returns `'fr'` when `document.documentElement.lang = 'fr'`.
  - `dates.test.ts`: with the locale overridden to `fr`, `formatShortDate(new Date('2026-09-12'))` returns `'12 sept.'`; with `en` it returns `'12 Sep'`.
  - `language.test.ts`: `saveLanguage('fr')` sends `PUT /api/v1/settings/language` with body `{"language":"fr"}`, and throws on a 500.
- [ ] **Step 2: Run** `pnpm --dir web test src/lib/i18n`. Expect it to FAIL because the modules are missing.
- [ ] **Step 3: Implement**
  - Configure `paraglideVitePlugin({ project: './project.inlang', outdir: './src/lib/paraglide', strategy: [...] })`, and make the CLI compile script use the same options. A clean checkout must then pass `pnpm --dir web verify` with nothing generated committed.
  - In `hooks.server.ts`, call `registerServerLocale`, then wrap `resolve` in `paraglideMiddleware` and replace `%paraglide.lang%` with the locale in `transformPageChunk`.
  - In `hooks.client.ts`, call `registerClientLocale`.
  - Replace the four hard-coded date formats with the `dates.ts` helpers.
- [ ] **Step 4: Run** `pnpm --dir web verify`. Expect PASS; existing English tests stay green because jsdom falls through to `en`.
- [ ] **Step 5: Commit** with `feat(web): wire Paraglide with the workspace language`.

### Task 3: Language switch, app shell, sign-in, error and unavailable pages

**Files:**
- Create: `web/src/lib/components/LanguageMenu.svelte`, `LanguageMenu.test.ts`, and `web/src/lib/i18n/errors.ts`
- Modify: `AppHeader.svelte` (replace the disabled "EN"), `routes/+layout.svelte`, `routes/+error.svelte`, `routes/login/*`, `AccessUnavailable.svelte`, `SiteFooter.svelte`, `ui/ConfirmDialog`, `ui/Toast`, `lib/auth.ts`, `lib/server/auth.ts`, `lib/server/settings.ts`, `lib/gallery/api.ts`, and any other call site that shows `data.error` or `error.message` from the API
- Test: the existing tests of these files, plus French cases

**Interfaces:**
- Consumes: `saveLanguage`, `getLocale` and `setLocale` from Task 2.
- Produces: `apiErrorMessage(status: number, field?: string): string` in `#lib/i18n/errors.js`. It returns a translated sentence for 401, 404, 409, 413, 422 (per field: `title`, `studio_name`, `logo`, `position`, `size`, `expires_at`, …), 429, 503, 507, and otherwise a generic "Something went wrong. Please try again." message. `GalleryApiError.message` and the auth and settings clients use it instead of the API's `error` text.

- [ ] **Step 1: Write the failing tests**
  - `LanguageMenu.test.ts`:
    - The button reads `EN`.
    - Its menu lists `English` and `Français`, and the current one is marked.
    - Choosing `Français` calls `saveLanguage('fr')` and then `setLocale('fr')`.
    - A failed save shows an error toast, does not call `setLocale`, and leaves the button on `EN`.
  - `errors.test.ts`: `apiErrorMessage(507)` in French is the French storage-full sentence, and an unknown 418 gives the generic message.
  - The `AccessUnavailable` and `+error` tests: with the locale set to `fr`, the French title renders and the English title is absent.
- [ ] **Step 2: Run** `pnpm --dir web test LanguageMenu errors AccessUnavailable error`. Expect it to FAIL.
- [ ] **Step 3: Implement**
  - Use the existing menu primitive (see `GallerySortMenu.svelte`).
  - A successful save calls `setLocale(lang)`, which reloads the page in the new language. That reload replaces the "Saved" toast; see the design's implementation choices.
  - Move every string in the listed files, including `aria-label`, `title`, `alt`, `<title>` and placeholder text, into `messages/*.json` with French translations.
- [ ] **Step 4: Run** `pnpm --dir web verify`. Expect PASS.
- [ ] **Step 5: Commit** with `feat(web): language switch and translated shell`.

### Tasks 4–6: Translate the screens

These three tasks share one shape: they consume `m`, `apiErrorMessage` and the `dates.ts` helpers.
1. Move every user-visible string (text, `aria-label`, `title`, `alt`, toasts, dialogs, empty states, plurals) into both message files.
2. Add one French render test per screen asserting two representative French strings, and that the screen's main English heading is absent.
3. Keep the existing English tests green.

For counts ("1 photo", "3 photos", "N finals · M downloaded"), use Paraglide's plural variants; French treats 0 and 1 as singular.

Each task ends with `pnpm --dir web verify`, which must PASS, and a commit.

- **Task 4: Dashboard and notifications.**
  - Files: `routes/dashboard/+page.svelte`, `EmptyWorkspace`, `GalleryCard`, `GalleryStateFilter`, `GallerySortMenu`, `NewGalleryDialog`, `NotificationBell`, `ExpiryBadge`, `lib/gallery/format.ts` (summaries, notification text), `lib/activity.ts`.
  - Commit: `feat(web): translate the dashboard and notifications`.
- **Task 5: Gallery workspace and settings.**
  - Files: `routes/dashboard/[id]/+page.svelte`, `GalleryHeader`, `GalleryTabs`, `JourneySteps`, `PhotosPanel`, `PhotoTile`, `lib/gallery/uploads.svelte.ts` messages, `FavoritesPanel`, `GallerySettings`, `ShareLink`, `CopyLink`, `PublishDialog`, `Archive`/`Delete`/`RestoreExpired` dialogs, `ClientPreview`, `Placeholder`, `routes/settings/+page.svelte`, `WatermarkEditor`, `CoverSignaturePicker`, `StudioSignature`.
  - Commit: `feat(web): translate the gallery and settings`.
- **Task 6: Client pages.**
  - Files: `routes/g/[token]/+page.svelte`, `ClientGallery`, `ProofGrid`, `ProofThumbnail`, `ProofViewer`, `FavoriteButton`, `ViewerButton`, `FilmStrip`, `CoverBackdrop`, the client empty states, the ZIP and download messages, and the delivered page.
  - Commit: `feat(web): translate the client pages`.

### Task 7: Sweep, layout check and docs

**Files:**
- Modify: `README.md` (status: English and French), `web/AGENTS.md` (rule: all user-visible text goes through `m.*` with both `en` and `fr` entries; dates through `#lib/i18n/dates.js`), and `docs/designs/2026-11-06-ui-languages-design.md` (implementation choices, if any changed)

- [ ] **Step 1: Sweep for leftover literals.**
  - Run `grep -rnE ">[^<{]*[A-Za-z]{3,}[^<{]*<|(aria-label|title|alt|placeholder)=\"[A-Za-z]" web/src --include=*.svelte | grep -v test`.
  - Expect only lines that hold the photographer's content, brand names or icons, and justify each one in the review.
- [ ] **Step 2: Layout check.**
  - Run the stack (see `README.md`) and switch to French.
  - Compare the top bar, dashboard, gallery and client page at 1280 px and 375 px with the pictures in `docs/designs/ui/screens/*` (including `client/cover-fr.*`). Nothing may clip or wrap badly.
  - Fix with existing tokens only.
- [ ] **Step 3: Update the docs** listed above.
- [ ] **Step 4: Run** all checks:
  - `cargo +nightly fmt --all`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --no-fail-fast` (the known libvips and logo failures are local only)
  - `pnpm --dir web verify`
- [ ] **Step 5: Commit** with `docs: English and French`.
