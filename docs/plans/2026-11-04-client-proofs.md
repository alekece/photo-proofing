# Client Proofs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A client holding a valid private link sees the gallery's ready proofs (thumbnail grid and preview viewer), and no client route can reach an original, an unready photo or another gallery's photo.

**Architecture:** Upload stores the displayed (EXIF-rotated) size. The database resolves a share token to the gallery's location and client view in one query. An API extractor, `ClientAccess`, authorises every `/galleries/shared/{token}` route with that query and picks the photo set from the gallery state. The SvelteKit client page loads the list server-side and points `<img>` at the client media routes.

**Tech Stack:** Rust (actix-web, sqlx/Postgres, libvips), SvelteKit/TypeScript (vitest, testing-library).

**Spec:** `docs/designs/2026-10-31-client-proofs-design.md` (issue #101)

## Global Constraints

- Unknown, malformed, unshared and expired links, and every unready, foreign or unknown photo id, are the same `404`.
- No client route takes a photo kind from the client; the API picks the set (`ClientPhotoSet::Proofs` only in this slice).
- No route returns a proofing original.
- The list is `no-store`; images are `image/jpeg`, `private` with a modest `max-age`, `nosniff`, `no-referrer`.
- Rotated orientations are EXIF 5 to 8; they swap width and height. 1 to 4, missing or invalid values keep them.
- No access log may record `/galleries/shared/` paths (existing rule; add none).
- Rust checks: those in `crates/AGENTS.md`. Web: `pnpm --dir web verify`.

## Review Focus

1. An image request after the link expires or the gallery is unshared: 404, even if the list loaded earlier. Pinned in Task 3.
2. A ready photo whose derivative file is missing on disk: 404, not 500. Pinned in Task 3.
3. A JPEG with no EXIF or an out-of-range orientation (0, 9): dimensions kept, upload accepted. Pinned in Task 1.
4. The list fetch failing after the gallery resolved (link expired between the two calls): the page shows the neutral unavailable page, not an error. Pinned in Task 4.
5. A masonry gallery with very tall or wide photos: the tile keeps the photo's real shape (width and height attributes), not a forced 3:2. Pinned in Task 4.

---

### Task 1: Store the displayed size at upload

**Files:**
- Modify: `crates/api/src/upload/jpeg.rs` (`inspect`, tests)
- Modify: `crates/database/src/photo.rs:31` (doc of `PhotoDimensions`)
- Modify: `docs/designs/2026-10-31-photo-upload-design.md` (one sentence where it says dimensions are parsed)

**Interfaces:**
- Produces: `inspect(path, max_pixels) -> Result<PhotoDimensions, ErrorResponse>` unchanged in signature; it now returns the displayed size.

- [ ] **Step 1: Write the failing test** in `jpeg.rs` tests: `inspection_applies_the_exif_orientation`. Build a 20×10 JPEG and insert an APP1 EXIF segment carrying orientation `n` (copy the `with_segment` and `exif` fixture helpers from `crates/image-worker/src/render.rs` tests; only IFD0 is needed). Assert:
  - orientation 6 and 8 → `(10, 20)`;
  - orientation 3 → `(20, 10)`;
  - no EXIF, orientation 0 and orientation 9 → `(20, 10)`.
- [ ] **Step 2: Run it and see it fail:** `cargo test -p pp-api inspection_applies_the_exif_orientation` → FAIL on orientation 6.
- [ ] **Step 3: Implement.** After reading width and height in `inspect`, swap them when `image.get_orientation()` is in `5..=8`. Validate the pixel count on the result (the product is the same). Change the `PhotoDimensions` doc to "Displayed dimensions (after the EXIF orientation), both non-zero." In the upload design, note that the stored size is the displayed one.
- [ ] **Step 4: Run** `cargo test -p pp-api upload` → PASS.
- [ ] **Step 5: Commit** `feat(api): store the displayed photo size at upload (#101)`.

### Task 2: Resolve a share token to its gallery, and list ready photos

**Files:**
- Modify: `crates/database/src/gallery/share.rs`, `crates/database/src/gallery.rs` (`find_shared`)
- Modify: `crates/database/src/photo.rs` (new type and two methods, tests)

**Interfaces:**
- Produces in `pp_database::gallery`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, FromRow)]
  pub struct SharedAccess {
      #[sqlx(flatten)] pub location: PhotoLocation,
      #[sqlx(flatten)] pub gallery: SharedGallery,
  }
  pub async fn find_shared(&self, token: &ShareToken) -> Result<Option<SharedAccess>, DatabaseError>
  ```
  Same `WHERE` as today. It selects `id AS gallery_id, workspace_id, title, layout, expires_at`.
- Produces in `pp_database::photo`:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
  pub struct ReadyPhoto { pub id: PhotoId, #[serde(flatten)] pub dimensions: PhotoDimensions }
  pub async fn list_ready(&self, location: PhotoLocation) -> Result<Vec<ReadyPhoto>, DatabaseError>
  pub async fn is_ready(&self, location: PhotoLocation, id: PhotoId) -> Result<bool, DatabaseError>
  ```
  `list_ready` orders by `created_at, id`.

- [ ] **Step 1: Write the failing tests.**
  - In `gallery.rs` tests, extend the existing `find_shared` tests: a shared gallery returns `location == PhotoLocation { workspace_id, gallery_id }` and the same `gallery` fields as before.
  - In `photo.rs` tests, add `list_ready_returns_only_ready_photos_of_that_gallery_oldest_first`. Use photos in all four states plus a ready photo in another gallery, with `TestRepository::force_photo_status` and `set_photo_created_at`. Assert the exact ids in order and their width and height.
  - Add `is_ready_is_false_for_unready_foreign_and_unknown_ids`.
- [ ] **Step 2: Run them and see them fail:** `cargo test -p pp-database --features tests photo:: gallery::` → fails to compile on the missing items.
- [ ] **Step 3: Implement** the type and the methods above, and make `show_shared_gallery` in `crates/api/src/routes/galleries.rs` return `.gallery` so the API still builds (Task 3 moves it behind the extractor).
- [ ] **Step 4: Run** `cargo test -p pp-database --features tests && cargo test -p pp-api shared` → PASS.
- [ ] **Step 5: Commit** `feat(database): resolve shared galleries and list ready photos (#101)`.

### Task 3: Client access extractor and client photo routes

**Files:**
- Create: `crates/api/src/extractors/client_access.rs`; register it in `crates/api/src/extractors.rs`
- Create: `crates/api/src/routes/galleries/shared.rs`. Move `show_shared_gallery` and its tests here from `galleries.rs`, and update the module doc in both files.
- Modify: `crates/api/src/routes/galleries.rs` (`mod shared;`, `.configure(shared::configure)` before `photos::configure`)
- Modify: `crates/api/src/routes/galleries/photos.rs`. Extract the storage read of `serve_derivative_photo` into a shared helper, and make the test helpers it needs `pub(super)` (`app`, `create_gallery`, `photo_in`, `Progress`, `store_derivative`, `jpeg`, `upload_request`, `call_request`).
- Modify: `crates/api/src/response.rs` (`ImageFile` headers)

**Interfaces:**
- Consumes: `GalleryRepository::find_shared`, `SharedAccess`, `PhotoRepository::{list_ready, is_ready}` (Task 2).
- Produces:
  ```rust
  /// Every client route takes this: the token from the `{token}` path segment resolved to a
  /// gallery the client may see now. Any failure is the same 404.
  pub(crate) struct ClientAccess(pub SharedAccess);
  impl FromRequest for ClientAccess { type Error = ErrorResponse; ... }
  impl ClientAccess { pub(crate) fn photo_set(&self) -> ClientPhotoSet }

  /// The photo set a client may see, chosen from the gallery state.
  pub(crate) enum ClientPhotoSet { Proofs }
  ```
  The extractor reads `request.match_info().get("token")`, parses a `ShareToken` (a failure is a 404) and calls `find_shared`. A database error goes through `ErrorResponse::from`.
- Routes, inside the `/galleries` scope:
  - `GET /shared/{token}`: unchanged response (`.gallery`), now through `ClientAccess`.
  - `GET /shared/{token}/photos`: `match photo_set { Proofs => list_ready }`, `no_store().no_referrer()`.
  - `GET /shared/{token}/photos/{photo_id}/{image:thumbnail|image}`: `ClientImage { Thumbnail, Image }`. For `Proofs`, `Thumbnail` maps to `Variant::Thumbnail` and `Image` to `Variant::Preview`. Return 404 unless `is_ready`.
- Shared helper in `photos.rs`: `pub(super) async fn read_image(state: &ApiState, key: &PhotoKey) -> Result<ImageFile, ErrorResponse>`. `NotFound` becomes a 404.
- `ImageFile` sends `Cache-Control: private, max-age=3600` and `Referrer-Policy: no-referrer` (a derivative never changes for its id).

- [ ] **Step 1: Write the failing tests** in `shared.rs`. Reuse the moved `show_shared_gallery` tests unchanged; they must still pass. Add:
  - `client_lists_only_ready_photos_with_dimensions`: ready, waiting, processing and failed photos. The list is exactly the ready ids as `{id, width, height}`, with `Cache-Control: no-store`.
  - `client_serves_thumbnail_and_image_of_a_ready_photo`: the bytes equal the stored `thumbnail` and `preview`, with `image/jpeg`, `private, max-age=3600`, `nosniff` and `no-referrer`.
  - `client_media_is_404_for_unready_foreign_unknown_and_missing_files`: waiting, failed, another gallery's ready photo, a random id, and a ready photo whose file was deleted.
  - `expired_and_unshared_links_deny_list_and_media`: an unshared gallery's would-be token (random), a malformed token, and a shared gallery whose `expires_at` a direct SQL `UPDATE` in the test sets to the past (the API may refuse a past expiry). List, thumbnail and image are all 404.
  - `no_client_route_serves_the_original`: `/photos/{id}/original` and `/photos/{id}/preview` are 404 for a ready photo whose original exists.
  - In `response.rs` or the existing photographer serving test, assert the new `Cache-Control` value.
- [ ] **Step 2: Run them and see them fail:** `cargo test -p pp-api shared` → FAIL (404 or compile errors for the missing routes).
- [ ] **Step 3: Implement** the extractor, the routes and the helper above.
- [ ] **Step 4: Run** `cargo test -p pp-api` → PASS, then the full Rust checks in `crates/AGENTS.md`.
- [ ] **Step 5: Commit** `feat(api): serve ready proofs to clients behind the private link (#101)`.

### Task 4: Show ready proofs on the client page

**Files:**
- Modify: `web/src/lib/gallery/types.ts`. Add `SharedPhoto { id: string; width: number; height: number }`, and extend `Proof` with `thumbnailUrl`, `width` and `height`.
- Create: `web/src/lib/gallery/proofs.ts` with `toProofs(token: string, photos: SharedPhoto[]): Proof[]`, tested in `proofs.test.ts`.
- Modify: `web/src/lib/server/galleries.ts`, adding `getSharedPhotos(fetcher, token): Promise<SharedPhoto[] | null>` with the same error contract as `getSharedGallery`. Test it in `galleries.test.ts`.
- Modify: `web/src/routes/g/[token]/+page.server.ts` and `+page.svelte`, which pass `proofs` to `ClientGallery`.
- Modify: `web/src/lib/components/ProofThumbnail.svelte`, which takes `layout`. Update `ProofGrid.svelte` and the existing tests and fixtures that build `Proof`.

**Interfaces:**
- Consumes: `GET /api/v1/galleries/shared/{token}/photos` and the media routes (Task 3).
- URLs are relative and same-origin, with the token `encodeURIComponent`-ed:
  - `thumbnailUrl`: `/api/v1/galleries/shared/{token}/photos/{id}/thumbnail`
  - `previewUrl`: `/api/v1/galleries/shared/{token}/photos/{id}/image`
  - `alt`: `Proof {n}`, 1-based.

- [ ] **Step 1: Write the failing tests.**
  - `toProofs` builds those exact URLs and alts from two photos and keeps the order and dimensions.
  - `getSharedPhotos` parses the list, forwards no cookie, returns `null` on 404, and fails with 503 on a network or server error (mirror the `getSharedGallery` tests).
  - `ProofThumbnail` uses `thumbnailUrl` as `src` and sets `width` and `height`. It has the `aspect-3/2` crop in the grid layout and not in masonry.
  - The page load returns `{ gallery: null, proofs: [] }` when the list is `null` after the gallery resolved.
- [ ] **Step 2: Run them and see them fail:** `pnpm --dir web test` → the new tests FAIL.
- [ ] **Step 3: Implement.** The page load fetches the gallery first. If it's `null`, it skips the list; if the list is `null`, it returns `gallery: null`. The viewer keeps using `previewUrl`.
- [ ] **Step 4: Run** `pnpm --dir web verify` → PASS.
- [ ] **Step 5: Commit** `feat(web): show ready proofs on the shared client page (#101)`.

### Task 5: Document the client routes

**Files:**
- Modify: `README.md`, only if its implementation-status section lists the client page or shared routes. Mark client proofs as shipped.

- [ ] **Step 1:** Check `README.md` for the current status wording of the client page and update it to match the shipped behaviour (ready proofs, reload for new ones).
- [ ] **Step 2: Commit** `docs: note client proofs in the README (#101)` (skip if nothing changed).
