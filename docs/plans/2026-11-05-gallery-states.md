# Gallery States and Dashboard Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Each gallery in the list carries its states, the list filters by one or several of them, and the dashboard shows and filters them.

**Architecture:** Three nullable timestamps on `galleries` plus `expires_at` give a `states` list computed in SQL and returned with every `Gallery`. `GET /api/v1/galleries` takes repeated `state` values combined with OR. The web app validates `states` with Valibot, keeps the filter in the URL beside `sort`, and renders chips, badges and a multi-select.

**Tech Stack:** Rust (actix-web, sqlx, Postgres), SvelteKit 3, Valibot, Vitest.

**Spec:** `docs/designs/2026-11-05-gallery-states-design.md` (issue #86).

## Global Constraints

- Progress state is exactly one of `draft`, `shared`, `delivered`; flags are `archived` and `expired`; `states` lists progress first, then flags in the order `archived`, `expired`.
- Progress: `delivered_at` set gives `delivered`, else `shared_at` set gives `shared`, else `draft`.
- `expired` is `expires_at` in the past by the database clock, evaluated per request and never stored.
- `state` filter: repeatable, plain OR across all five values, unknown value gives 400, always scoped to the session's workspace, combines with `sort`.
- Migration adds `shared_at`, `delivered_at`, `archived_at` (nullable `timestamptz`); backfill `shared_at = updated_at` where `share_token IS NOT NULL`; `share()` sets `shared_at` when it creates the link and keeps `updated_at` untouched.
- Nothing in this slice sets `delivered_at` or `archived_at`; tests write them with SQL.
- Out of scope: card counts, activity chips, hero thumbnail, pagination, server-side title search.
- Rust: format with `cargo +nightly fmt --all`; clippy `-D warnings`; Rust code follows `crates/AGENTS.md`. Web: `pnpm --dir web verify`. Readability rules from the user: blank line before control flow, after `let` groups and before the return expression.

## Review Focus

- A gallery with an expiry date in the past and no share link: states are `["draft", "expired"]`, and `?state=expired` lists it.
- `?state=shared&state=shared` (a repeated value) behaves like one value; `?state=` (empty) is a 400, not "no filter".
- `?state=archived&sort=title`: the filter and every sort order combine and stay stable.
- Another workspace's galleries never appear for any filter.
- A gallery shared before the migration keeps `shared_at` equal to its old `updated_at` and still reports `shared`.
- The dashboard URL `?state=bogus&state=shared` keeps `shared` and drops `bogus` instead of failing.
- Search and filter together on the dashboard: the no-results screen appears, while a workspace with no galleries still shows the empty dashboard.

---

### Task 1: Gallery states in the database

**Files:**
- Create: `crates/database/migrations/0008_gallery_states.sql`
- Modify: `crates/database/src/gallery.rs` (struct, `COLUMNS`, `share`, tests)

**Interfaces:**
- Produces: `pub enum GalleryState { Draft, Shared, Delivered, Archived, Expired }` with `#[serde(rename_all = "lowercase")]`, `Debug, Clone, Copy, PartialEq, Eq, Serialize`; `Gallery.states: Vec<GalleryState>` (serialized as `states`), built in `FromRow` from `shared_at`, `delivered_at`, `archived_at` and an `expired` boolean selected as `COALESCE(expires_at <= now(), false) AS expired`. Every query that returns a `Gallery` goes through `COLUMNS`, so `create`, `get`, `share` and `update` carry `states` too.

- [ ] **Step 1: Write the failing tests** in the `gallery.rs` test module, following the existing database test style (transaction-scoped schema, SQL to set columns):
  - `states_follow_the_timestamps`: a new gallery is `[Draft]`; after `share()` it is `[Shared]`; after setting `delivered_at` it is `[Delivered]`; after also setting `archived_at` it is `[Delivered, Archived]`.
  - `expired_is_a_flag_on_any_progress_state`: with `expires_at` in the past a draft is `[Draft, Expired]`; a future `expires_at` adds nothing.
  - `share_records_when_the_link_was_created`: `shared_at` is set by the first `share()`, unchanged by the second, and `updated_at` is untouched.
  - `migration_backfills_shared_at`: follow the existing migration regression test (apply `0007`, insert a shared and a draft gallery, apply `0008`, compare `shared_at` to `updated_at` and null).
- [ ] **Step 2: Run to verify failure:** `cargo test -q -p pp-database gallery::` — Expected: FAIL (missing column / `states`). Needs `DATABASE_URL=postgres://pp:pp-local-only@localhost:55432/pp`.
- [ ] **Step 3: Write the migration and implement.** `0008_gallery_states.sql` adds the three columns and the backfill. Add `GalleryState`, the `states` field, the new `COLUMNS` entries and the `from_row` derivation (progress first, then `archived`, then `expired`). In `share()` set `shared_at = COALESCE(shared_at, now())`.
- [ ] **Step 4: Run to verify pass:** `cargo test -q -p pp-database` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(database): derive gallery states from timestamps`.

---

### Task 2: Filter the list by state

**Files:**
- Modify: `crates/database/src/gallery.rs` (`list`), `crates/api/src/routes/galleries.rs` (`ListQuery`, `list_galleries`, tests), `crates/api/src/tests.rs` if a helper is needed
- Modify: `README.md` and the route's module doc (list endpoint description)

**Interfaces:**
- Consumes: `GalleryState`, `Gallery.states` from Task 1.
- Produces: `GalleryRepository::list(&self, workspace_id: WorkspaceId, sort: GallerySort, states: &[GalleryState]) -> Result<Vec<Gallery>, DatabaseError>`; an empty slice means no filter. `GalleryState` parses from its lowercase name (`FromStr` or `Deserialize`, whichever the query parsing uses).

- [ ] **Step 1: Write the failing tests.**
  - Database: `list_filters_by_any_chosen_state` (one value, several values OR, a flag value, every sort order, repeated value equals single); `list_filter_stays_in_the_workspace`.
  - API: `GET /api/v1/galleries?state=shared&state=expired` returns the matching galleries with `states`; `?state=bogus` and `?state=` return 400; `?state=archived&sort=title` combines; a signed-in user never sees another workspace's galleries; the existing `?sort=oldest` 400 still holds.
- [ ] **Step 2: Run to verify failure:** `cargo test -q -p pp-database -p pp-api galleries` — Expected: FAIL.
- [ ] **Step 3: Implement.** In `list`, build the `WHERE` clause from fixed SQL fragments per `GalleryState` (never from user text), joined with OR inside parentheses, then `AND workspace_id=$1`. In the API, parse the query so `state` can repeat: read `Vec<(String, String)>` pairs from the query string and build `sort` and `states` from them, so an unknown or empty value of either gives the same 400 the current `Query<ListQuery>` rejection gives. Update the module doc line (`GET /?sort=…&state=…`) and the README endpoint text.
- [ ] **Step 4: Run to verify pass:** `cargo test -q --workspace --all-features`, then `cargo +nightly fmt --all` and `cargo clippy -q --workspace --all-targets --locked --all-features -- -D warnings` — Expected: all clean.
- [ ] **Step 5: Commit** `feat(api): filter the gallery list by state`.

---

### Task 3: Web schema, URL parsing and server load

**Files:**
- Modify: `web/src/lib/gallery/schemas.ts`, `web/src/lib/gallery/schemas.test.ts`, `web/src/lib/server/galleries.ts` (+ its test), `web/src/routes/dashboard/+page.server.ts`, `web/src/routes/dashboard/page.server.test.ts`; update `Gallery` fixtures in web tests that now need `states`.

**Interfaces:**
- Produces in `schemas.ts`: `galleryStateSchema = v.picklist(['draft','shared','delivered','archived','expired'])`; `type GalleryState`; `states: v.array(galleryStateSchema)` added to `gallerySchema`; `parseGalleryStates(values: unknown[]): GalleryState[]` which keeps valid values, drops unknown ones and removes duplicates, preserving order.
- Produces in `server/galleries.ts`: `listGalleries(fetcher, cookie, sort, states: GalleryState[])` sends `sort` and one `state` parameter per value.
- Produces in the dashboard load result: `states: GalleryState[]` next to `sort`.

- [ ] **Step 1: Write the failing tests.** Schema: a gallery with `states` is accepted, an unknown state or a missing `states` is rejected. `parseGalleryStates(['shared','bogus','shared'])` gives `['shared']`. Server load: `?state=shared&state=bogus` requests the API with only `state=shared` and returns `states: ['shared']`; no `state` gives `[]`; a bad body is still the 503 path.
- [ ] **Step 2: Run to verify failure:** `pnpm --dir web test` — Expected: FAIL.
- [ ] **Step 3: Implement** the schema, `parseGalleryStates`, the `listGalleries` parameter and the load changes (`url.searchParams.getAll('state')`).
- [ ] **Step 4: Run to verify pass:** `pnpm --dir web check` and `pnpm --dir web test` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(web): read gallery states and the state filter`.

---

### Task 4: Dashboard chips, badges, filter and no-results

**Files:**
- Create: `web/src/lib/components/GalleryStateFilter.svelte` (+ `GalleryStateFilter.test.ts`)
- Create: `web/src/lib/gallery/states.ts` (display labels and the `progress`/`flags` split)
- Modify: `web/src/routes/dashboard/+page.svelte`, `web/src/routes/dashboard/page.test.ts`

**Interfaces:**
- Consumes: `GalleryState`, `parseGalleryStates`, `data.states` from Task 3; the existing `ExpiryBadge` for the expiry display where it already applies.
- Produces: `GalleryStateFilter` with props `{ selected: GalleryState[]; onchange: (next: GalleryState[]) => void }`, a labelled group of five checkboxes (Draft, Shared, Delivered, Archived, Expired) reachable by keyboard; `states.ts` exports `STATE_LABELS: Record<GalleryState, string>`.

- [ ] **Step 1: Write the failing tests.**
  - `GalleryStateFilter`: lists all five states; checking one calls `onchange` with it appended, unchecking removes it; selected ones are checked.
  - Dashboard: changing the filter navigates to `?sort=…&state=…` for every chosen state; each card shows its progress chip and any flag badges; with `?state=archived` matching nothing, the "No galleries match" screen appears with a way to clear search and filter, while a workspace with no galleries still shows the golden-hour empty state; title search applies on top of the filter.
- [ ] **Step 2: Run to verify failure:** `pnpm --dir web test` — Expected: FAIL.
- [ ] **Step 3: Implement** the filter component and states helper, wire them into `+page.svelte` (keep sort in the URL when the filter changes and the filter when the sort changes), render chips and badges on cards following the dashboard mockup (`docs/designs/assets/pages/dashboard.html`), and make "Clear" reset search and filter. Show the filter and search whenever the workspace has galleries, including when the filter matches none.
- [ ] **Step 4: Run to verify pass:** `pnpm --dir web verify` — Expected: PASS.
- [ ] **Step 5: Commit** `feat(web): show gallery states and filter the dashboard`.

---

## Completion

After the four tasks: README mentions the `state` parameter and the `states` field (done in Task 2), the PR references #86, and the plan file is removed before the final merge request, per the repository's plan cleanup rule.
