# Private client link and client page Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A photographer shares a gallery through an unguessable fixed link, and anyone holding it sees the persisted gallery on a client page, or one neutral "unavailable" page.

**Architecture:** A nullable unique `galleries.share_token` (128 random bits, 22 URL-safe base64 characters, plaintext). `POST /galleries/{id}/share` is idempotent. A public `GET /shared/{token}` resolves the token with expiry checked in the same query, through a `SharedAccess` extractor that later client routes reuse. SvelteKit `/g/[token]` loads from that route; the photographer's gallery page gets a Share control.

**Tech Stack:** Rust (Actix, sqlx, PostgreSQL, `rand`, `base64`), SvelteKit, Tailwind, shadcn-svelte, Vitest.

**Spec:** `docs/superpowers/specs/2026-10-30-private-client-link-design.md` (issue #65; SSE is #80, photographer preview is #81).

## Global Constraints

- Follow `AGENTS.md`: nightly fmt, imports at module top (types by name, functions via their module), blank-line rules, `module.rs` + `module/child.rs`, newtypes for meaningful values, comments explain why.
- Token: 128 random bits from `rand::rng()`, 22 unpadded URL-safe base64 characters, canonical encoding only, never printed by `Debug`.
- Unknown, malformed and expired links produce byte-identical 404 `ErrorResponse` responses with `Cache-Control: no-store` and `Referrer-Policy: no-referrer`.
- Expiry is checked with the database clock (`expires_at IS NULL OR expires_at > now()`).
- New migration `0003_share_tokens.sql`; `0002` is not edited.
- No SSE, no photos or hero, no rotation or unshare, no preview, no cookies for clients.
- Unavailable copy is neutral and tells the client to contact the photographer; it never says "expired".
- Checks: `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked` (needs `DATABASE_URL`), and in `web/`: `pnpm test`, `pnpm check`. Adding dependencies needs one run without `--locked` to update `Cargo.lock`.

## Review Focus

- A malformed token (wrong length, non-alphabet characters, non-canonical trailing bits, case-altered variants of a real token) must return the same 404 as an unknown one, without a database error. Pinned in Task 2.
- Expiry in the past, in the future, cleared and extended must flip access without a new link. Pinned in Task 1.
- Two concurrent share calls must converge on one token. Pinned in Task 1.
- A gallery with no title, or a 255-character title with no spaces, must render without breaking the client page layout. Pinned in Task 3.
- Clipboard API missing or rejecting must leave the link selectable and show feedback instead of throwing. Pinned in Task 4.

---

### Task 1: Share token and repository

**Files:**
- Create: `crates/database/migrations/0003_share_tokens.sql`
- Create: `crates/database/src/gallery/share.rs`
- Modify: `crates/database/src/gallery.rs` (add `mod share;`, re-export, `COLUMNS`, `Gallery`, new repository methods, tests), `crates/database/Cargo.toml` (`rand`, `base64`, versions as in `crates/api/Cargo.toml`)

**Interfaces:**
- Produces:
  - `pub struct ShareToken(String)` in `gallery/share.rs`, re-exported from `pp_database::gallery`: `ShareToken::generate() -> Self`, `as_str(&self) -> &str`, `impl FromStr for ShareToken { type Err = ParseShareTokenError }`, `Serialize` (as a string), redacted `Debug`, `sqlx` encode/decode as text.
  - `pub struct ParseShareTokenError` (`Debug`, `Display`, `Error`).
  - `pub struct SharedGallery { pub title: Option<String>, pub layout: GalleryLayout, pub expires_at: Option<Timestamp> }` (`Serialize`, `FromRow`).
  - `Gallery.share_token: Option<ShareToken>` (serialized as `share_token`, `null` when unshared).
  - `GalleryRepository::share(&self, workspace_id: WorkspaceId, id: GalleryId) -> Result<Option<Gallery>, sqlx::Error>`: returns the gallery with its token set; `None` for another workspace's gallery. Does not change `updated_at`.
  - `GalleryRepository::find_shared(&self, token: &ShareToken) -> Result<Option<SharedGallery>, sqlx::Error>`.

- [ ] **Step 1: Write failing tests** in `gallery.rs` `mod tests` (`#[sqlx::test(migrations = "./migrations")]`, existing `workspace`/`changes` helpers):
  - `sharing_creates_one_fixed_token_per_gallery`: a new gallery has `share_token == None`; `share` returns a gallery with `Some(token)`; a second `share` returns the identical token; `updated_at` is unchanged; `get` shows the token.
  - `concurrent_shares_converge_on_one_token`: `tokio::join!` of eight `share` calls all return the same token.
  - `another_workspace_cannot_share`: `share(other, id)` is `None` and the gallery stays unshared.
  - `find_shared_ignores_unknown_and_unshared`: an unknown `ShareToken::generate()` and a gallery that was never shared both give `None`.
  - `find_shared_enforces_expiry_at_request_time`: no expiry → found; `expires_at` in the past (set through `update`) → `None`; extended into the future → found with the same token; cleared → found. The result carries `title`, `layout` and `expires_at` only.
  - `share_token_check_rejects_malformed_values`: a raw `UPDATE galleries SET share_token = 'short'` fails.
  - In `share.rs`: `parsing_accepts_only_canonical_tokens` (a generated token round-trips; wrong length, `+`, `/`, padding, a 22-character value with non-canonical final character, and the empty string all fail) and `debug_does_not_print_the_token`.

- [ ] **Step 2: Run** `cargo test -p pp-database` (without `--locked` once, after adding the dependencies). Expected: compile failure or FAIL.

- [ ] **Step 3: Implement.** The migration adds `share_token text` with `CHECK (share_token ~ '^[A-Za-z0-9_-]{22}$')` and a `UNIQUE` constraint. `ShareToken` mirrors the canonical-parse approach of `crates/api/src/extractors/session_token.rs` (decode, require 16 bytes, re-encode and compare). `share` is one statement, `UPDATE galleries SET share_token = COALESCE(share_token, $3) WHERE id=$1 AND workspace_id=$2 RETURNING {COLUMNS}`, binding a fresh `ShareToken::generate()`; the row lock makes concurrent calls converge. `find_shared` selects `title,layout,expires_at` `WHERE share_token=$1 AND (expires_at IS NULL OR expires_at > now())`. Add `share_token` to `COLUMNS` and to `Gallery`'s `FromRow`.

- [ ] **Step 4: Run** `cargo test -p pp-database --locked`. Expected: PASS.

- [ ] **Step 5: Commit** `feat: store a private share token on galleries (#65)`

---

### Task 2: Share and shared routes, `SharedAccess`

**Files:**
- Create: `crates/api/src/extractors/shared_access.rs`, `crates/api/src/routes/shared.rs`
- Modify: `crates/api/src/extractors.rs`, `crates/api/src/routes.rs`, `crates/api/src/api.rs` (`.configure(routes::shared::configure)` inside `/api/v1`), `crates/api/src/routes/galleries.rs` (share route, module docs, tests), `crates/api/src/response.rs` (`no_referrer()`)

**Interfaces:**
- Consumes: Task 1 `ShareToken`, `SharedGallery`, `GalleryRepository::{share, find_shared}`.
- Produces:
  - `POST /api/v1/galleries/{id}/share` → 200 `Gallery` JSON including `share_token`, `no-store`; 404 for foreign, unknown or malformed ids; 401 without a session; 403 without the configured `Origin`.
  - `GET /api/v1/shared/{token}` → 200 `{ "title": string|null, "layout": "grid"|"masonry", "expires_at": string|null }`; otherwise 404 `ErrorResponse::not_found()`.
  - `pub(crate) struct SharedAccess(pub SharedGallery)` implementing `FromRequest<Error = ErrorResponse>`: reads the `token` path segment; a parse failure is `ErrorResponse::not_found()` with no database call; `find_shared` returning `None` is `not_found()`; a database error is `ErrorResponse::unavailable()`. Its doc comment says it is the single place client routes authorize, and that no request logging exists today, so any added access log must redact `/shared/` paths.
  - `ResponseBuilderExt::no_referrer(&mut self) -> &mut Self` setting `Referrer-Policy: no-referrer`.

- [ ] **Step 1: Write failing tests** (reuse the `app_state`, `call`, `sign_up`, `post_gallery` helpers of `galleries.rs`; if `shared.rs` tests need them, move the helpers to a shared `#[cfg(test)]` module rather than copying):
  - `sharing_returns_the_same_link_and_requires_session_and_origin`: a second share returns the same `share_token`; no cookie → 401; wrong `Origin` → 403; a gallery of another workspace (created through the repository on a second workspace) → 404.
  - `shared_page_returns_only_client_fields`: keys are exactly `title`, `layout`, `expires_at`; `no-store` and `no-referrer` headers are present; works with no cookie and no `Origin`.
  - `unknown_malformed_and_expired_links_are_indistinguishable`: status, headers and body are equal for an unknown valid-shape token, `"short"`, a 22-character non-canonical value, a case-altered variant of a real token, and a real token whose gallery expired through `update`.
  - `extending_the_expiry_restores_access_at_the_same_link`.

- [ ] **Step 2: Run** `cargo test -p pp-api --locked`. Expected: FAIL (routes absent).

- [ ] **Step 3: Implement** the routes and extractor with the signatures above. `shared::configure` mounts `web::scope("/shared")` with `#[get("/{token}")]`. `SameOrigin` already allows safe methods, so the middleware is unchanged.

- [ ] **Step 4: Run** `cargo test --workspace --locked` and `cargo clippy --workspace --all-targets --locked -- -D warnings`. Expected: PASS.

- [ ] **Step 5: Commit** `feat: share galleries and resolve private links (#65)`

---

### Task 3: Client page backed by the API

**Files:**
- Create: `web/src/routes/g/[token]/+page.server.ts`, `web/src/routes/g/[token]/+page.svelte` (moved from `g/[gallery]`), `web/src/lib/components/ClientGallery.svelte`
- Modify: `web/src/lib/server/galleries.ts`, `web/src/lib/gallery/types.ts`, `web/src/lib/components/ProofGrid.svelte` (+ its test), `web/src/lib/components/AccessUnavailable.svelte`, `web/src/routes/routes.test.ts`
- Delete: `web/src/lib/mock/galleries.ts`, `web/src/lib/gallery/access.ts` and `access.test.ts`, `web/src/lib/gallery/load.ts`, `web/src/lib/components/GalleryStateNotice.svelte`, the old `g/[gallery]` directory. Keep `mock/placeholder.ts` only if tests still use it as a fixture.

**Interfaces:**
- Consumes: `GET /api/v1/shared/{token}` from Task 2.
- Produces:
  - `types.ts`: `SharedGallery { title: string | null; layout: 'grid' | 'masonry'; expires_at: string | null }` and the existing `Proof`; the old state types are removed.
  - `getSharedGallery(fetcher: typeof fetch, token: string): Promise<SharedGallery | null>` in `lib/server/galleries.ts`: 404 → `null`; a network error or any other non-OK status → SvelteKit `error(503, ...)`; uses `PP_API_INTERNAL_URL` like the other helpers and forwards no cookie.
  - `ClientGallery.svelte` props `{ gallery: SharedGallery; proofs?: Proof[] }` (default `[]`): heading (title, or `UNTITLED_GALLERY` from `lib/gallery/format.ts`), `ExpiryBadge` only when `expires_at` is set, the empty state "The photographer hasn't added photos yet. Check back soon." when there are no proofs, otherwise `ProofGrid` and `ProofViewer`. #81 reuses it.
  - `ProofGrid` gains `layout: 'grid' | 'masonry'`.
  - Page data `{ gallery: SharedGallery | null }`; `null` renders `AccessUnavailable`.

- [ ] **Step 1: Write failing tests:**
  - `getSharedGallery`: 200 parses; 404 returns `null`; a thrown fetch and a 500 both raise a 503.
  - `routes.test.ts`: `gallery: null` shows the unavailable text and no gallery data; a gallery shows the title, shows the expiry badge only when set, and shows the empty state; an untitled gallery shows "Untitled gallery"; a 255-character title without spaces wraps (assert `overflow-wrap` in the heading's computed style or class).
  - `ProofGrid.test.ts`: `layout="grid"` and `layout="masonry"` both render every proof in DOM order with accessible names and expose `data-layout`; keyboard activation still opens the viewer.
  - `AccessUnavailable` text does not contain "expired".

- [ ] **Step 2: Run** `pnpm test` in `web/`. Expected: FAIL.

- [ ] **Step 3: Implement.** The page server load sets `Cache-Control: no-store` and `Referrer-Policy: no-referrer` with `setHeaders` and returns `{ gallery: await getSharedGallery(fetch, params.token) }`. Keep `<meta name="referrer" content="no-referrer">` and `noindex` in the page head. Masonry may use CSS columns as long as DOM order stays the reading order. `AccessUnavailable` copy: "This gallery isn't available" and "Please contact your photographer."

- [ ] **Step 4: Run** `pnpm test` and `pnpm check` in `web/`. Expected: PASS, and `grep -rn "mock/galleries\|gallery/access\|GalleryStateNotice\|loadClientGallery" web/src` prints nothing.

- [ ] **Step 5: Commit** `feat: render the client gallery page from the API (#65)`

---

### Task 4: Share control on the photographer's gallery page

**Files:**
- Create: `web/src/lib/components/ShareLink.svelte`, `web/src/lib/components/ShareLink.test.ts`
- Modify: `web/src/lib/gallery/api.ts` (+ `api.test.ts`), `web/src/routes/dashboard/[id]/+page.svelte` (+ `page.test.ts`)

**Interfaces:**
- Consumes: the share route from Task 2.
- Produces:
  - `Gallery.share_token: string | null` in `api.ts`; `shareGallery(id: string, fetcher: typeof fetch = fetch): Promise<Gallery>` → `POST /api/v1/galleries/{id}/share`.
  - `ShareLink.svelte` props `{ gallery: Gallery; origin: string; share?: typeof shareGallery }`: a Share button when `share_token` is null; once shared (or already shared), a read-only input with the link `${origin}/g/${token}`, a Copy button, and the warning "This gallery has no photos yet. Clients will see an empty gallery." (always shown until #66 supplies a photo count; comment this). A failed share shows an accessible alert and keeps the button usable.

- [ ] **Step 1: Write failing tests:**
  - `api.test.ts`: `shareGallery` posts to the share path and returns the gallery with `share_token`.
  - `ShareLink.test.ts`: unshared shows Share only; clicking calls `share` once, then shows the link, Copy and the warning; an already shared gallery shows the link without calling `share`; Copy writes the exact link to `navigator.clipboard.writeText` and announces "Copied"; with the clipboard missing or rejecting, the input is selected and the message asks to copy manually, without throwing; a failed `share` shows the alert and no link; every control has an accessible name and is keyboard reachable.
  - `dashboard/[id]/page.test.ts`: the page renders the Share control for the loaded gallery, with the link built from `page.url.origin`.

- [ ] **Step 2: Run** `pnpm test` in `web/`. Expected: FAIL.

- [ ] **Step 3: Implement** the component with the existing `Button` and `Input` from `lib/components/ui` and mount it in the gallery page. Copy tries `navigator.clipboard?.writeText`, falling back to `input.select()`.

- [ ] **Step 4: Run** `pnpm test` and `pnpm check` in `web/`. Expected: PASS.

- [ ] **Step 5: Commit** `feat: add a share control to the gallery page (#65)`

---

### Task 5: Docs and full verification

**Files:**
- Modify: `README.md` (implementation status for sharing and the client page); `docs/superpowers/specs/2026-10-30-private-client-link-design.md` (replace "a test asserts the token is absent from logs" with: no request logging exists today, `Debug` is redacted, and the extractor documents the redaction rule); `docs/releases/MVP.md` only if a line states this slice's behaviour (do not change scope).

- [ ] **Step 1: Run** `cargo +nightly fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, and in `web/`: `pnpm test` and `pnpm check`. Expected: all clean.
- [ ] **Step 2: Exercise it locally:** run the API and web app against a scratch PostgreSQL, sign in, create and share a gallery, open `/g/{token}` signed out (title and empty state), set an expiry in the past (the unavailable page), and open a garbage token (an identical page). Report anything that could not run.
- [ ] **Step 3: Update the docs** as listed and commit `docs: record the private client link slice (#65)`.
- [ ] **Step 4:** The plan file stays until review is finished and is deleted before the final merge request, per `AGENTS.md`.
