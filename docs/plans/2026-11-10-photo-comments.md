# Comments on proof photos Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clients write, edit and delete comments on proof photos; the photographer reads and replies in her viewer; client comments reach her bell.

**Architecture:** One `photo_comments` table (soft delete) behind a `CommentRepository` in the `database` crate; symmetric client and photographer routes in a new `comments` API module; a `CommentsClient` plus `CommentThread` component in the web app, shown in a closed-by-default panel of `ProofViewer`. The bell reuses `record_notification` with a new `comment` kind, so no new event channel.

**Tech Stack:** Rust (actix-web, SQLx/PostgreSQL), SvelteKit 5, Paraglide, Vitest.

**Spec:** [`docs/designs/2026-11-10-photo-comments-design.md`](../designs/2026-11-10-photo-comments-design.md), issue [#321](https://github.com/alekece/photo-proofing/issues/321). Read `crates/AGENTS.md` and `web/AGENTS.md` first (no free functions, blank-line rules, PascalCase components, every string in `en.json` and `fr.json`).

## Global Constraints

- Comment text: plain text, trimmed, 1 to 500 characters (`MAX_BODY_CHARS = 500`), a fixed constant, shown as text never HTML.
- Authors are exactly `client` and `photographer`. Comments are keyed by photo, not by photo kind, but every route accepts **proof** photos only.
- Client routes reuse `ClientAccess`. **Every** client comment route (read and write) answers 409 `gallery_delivered` once the gallery is delivered; unknown, unready-link, foreign or non-proof photos are the usual 404. No new error code.
- Deleting marks `deleted_at` and keeps the text. A client is **never** sent the text of a deleted comment (`body: null`); the photographer is. Deleted comments are excluded from counts and from notifications. Edits keep only the latest text and set `edited_at`.
- Anyone on the link may edit or delete any client comment; the photographer edits and deletes only her own replies. A comment cannot be edited once deleted.
- Only a **new client comment** records a notification (kind `comment`, count 1). Edits, deletes and photographer replies do not.
- No client notification, email, formatting, attachments or history. Thread closed by default; moving to another photo or pressing Escape closes it (Escape closes the thread before the viewer).
- Abuse limits, configuration with documented defaults (`[comments]` in `config.example.toml`, env `PP_COMMENTS_*`): `min_interval = "2s"` (`PP_COMMENTS_MIN_INTERVAL`, bounded 1 s to 1 min) and `max_per_photo = 100` (`PP_COMMENTS_MAX_PER_PHOTO`, non-zero `u16`). Client writes (post, edit, delete) are limited to one per `min_interval` **per gallery**, in memory; a faster one gets 429 `busy` (`ErrorResponse::busy(remaining)`). A client post on a photo already holding `max_per_photo` comments (deleted ones included) gets 409 `thread_full`. The photographer is exempt from both.
- Client responses carry `no-store` and `no-referrer` like the other client routes.

## Review Focus

- Client `GET` of a thread containing a deleted comment must not leak its text (Task 3).
- Whitespace-only, empty and 501-character bodies are rejected; exactly 500 characters is accepted (Task 1).
- Client cannot edit or delete a photographer reply, nor the photographer a client comment, nor anything in another gallery or workspace (Tasks 1, 3).
- A client write inside the interval gets 429 with `Retry-After`, and the same call after the interval succeeds; a different gallery is unaffected; the photographer is never limited (Task 3).
- The 101st comment on one photo is refused for a client (409 `thread_full`) but accepted from the photographer (Task 1).
- Deleting an already deleted comment is a no-op success; editing a deleted one is a 404 (Task 1).
- Deleting a photo removes its comments, so no orphan counts remain (Task 1).

---

### Task 1: Comments in the database

**Files:**
- Create: `crates/database/migrations/0028_photo_comments.sql`, `crates/database/src/comments.rs`
- Modify: `crates/database/src/lib.rs` (add `pub mod comments;`), `crates/database/src/notifications.rs` (`NotificationKind::Comment`)

**Interfaces:**
- Produces, in `pp_database::comments`:
  - `pub const MAX_BODY_CHARS: usize = 500;`
  - `pub struct CommentId(pub Uuid)` (same derives as `NotificationId`).
  - `pub enum CommentAuthor { Client, Photographer }`, serde and sqlx `snake_case` text.
  - `pub struct CommentBody` with `TryFrom<String>` (trims, enforces 1..=`MAX_BODY_CHARS` chars, error `CommentBodyError` via `thiserror`) and `Deserialize` through it; `as_str()`.
  - `pub struct Comment { id: CommentId, photo_id: PhotoId, author: CommentAuthor, body: Option<String>, created_at: Timestamp, edited_at: Option<Timestamp>, deleted_at: Option<Timestamp> }` (`Serialize`).
  - `pub struct CommentCount { photo_id: PhotoId, count: usize }` (`Serialize`).
  - `pub enum CommentChange<T> { Applied(T), NotFound, Delivered, Full }` (`Full` only from `add`).
  - `CommentRepository` (`Repository<'db>`, `Options = ()`) with:
    - `thread(&self, location: PhotoLocation, photo: PhotoId, viewer: CommentAuthor) -> Result<Option<Vec<Comment>>, DatabaseError>` (`None` when not a proof of that gallery; oldest first; deleted comments have `body: None` for `Client` viewers).
    - `counts(&self, location: PhotoLocation) -> Result<Vec<CommentCount>, DatabaseError>` (proofs with at least one non-deleted comment).
    - `add(&self, location, photo: PhotoId, author: CommentAuthor, body: &CommentBody, client_limit: NonZeroU16) -> Result<CommentChange<Comment>, DatabaseError>` (`Full` when `author == Client` and the photo already has `client_limit` comments, deleted ones included).
    - `edit(&self, location, id: CommentId, author: CommentAuthor, body: &CommentBody) -> Result<CommentChange<Comment>, DatabaseError>`
    - `delete(&self, location, id: CommentId, author: CommentAuthor) -> Result<CommentChange<()>, DatabaseError>`
  - `Delivered` is returned only for `author == Client` on a delivered gallery; a photographer may always write on her proofs.
- `NotificationKind::Comment` (serde `comment`).

- [ ] **Step 1: Write the failing tests** in `comments.rs` `mod tests` (`#[sqlx::test]`, fixture like `favorite_events.rs`: workspace, gallery, one proof photo, one final photo via `TestRepository`). Names and assertions:
  - `a_client_comment_appears_in_the_thread_and_counts`: `add` then `thread` returns one comment with `author == Client`, `body == Some("Nice")`, `edited_at == None`; `counts` returns `[CommentCount { photo_id, count: 1 }]`.
  - `bodies_are_trimmed_and_bounded`: `CommentBody::try_from` rejects `""`, `"  \n "` and a 501-char string, accepts exactly 500 chars and trims `" hi "` to `"hi"`.
  - `only_proofs_take_comments`: `add` on the final photo, on a photo of another gallery and on a random id each give `NotFound`; `thread` gives `None`.
  - `a_delivered_gallery_refuses_clients_but_not_the_photographer`: after setting `delivered_at`, `add`, `edit` and `delete` as `Client` give `Delivered`; `add` as `Photographer` is `Applied`.
  - `authors_only_change_their_own_comments`: a `Photographer` `edit`/`delete` of a client comment and a `Client` one of a photographer reply give `NotFound`; any client may edit any client comment, which sets `edited_at`.
  - `deleting_keeps_the_text_for_the_photographer_only`: after `delete`, `thread(.., Client)` has `body == None` and `deleted_at.is_some()`, `thread(.., Photographer)` keeps `body == Some(..)`; `counts` is empty; a second `delete` is `Applied(())`; `edit` is `NotFound`.
  - `deleting_a_photo_removes_its_comments`: delete the photo row, then `SELECT count(*) FROM photo_comments` is 0.
  - `a_full_thread_refuses_clients_only`: with `client_limit = 3`, three client `add`s apply and the fourth is `Full`; a deleted comment still counts; a `Photographer` `add` is `Applied`.
  - `only_a_new_client_comment_notifies`: with `NotificationRepository::list`, a client `add` gives one unread `Comment` notification with `count == 1` and a second `add` makes it 2; photographer `add`, `edit` and `delete` leave the count unchanged.
- [ ] **Step 2: Run** `cargo test -p pp-database comments` (needs the test database as for other `#[sqlx::test]` tests; see README). Expected: compile failure, then FAIL.
- [ ] **Step 3: Write the migration.** Table `photo_comments(id uuid PK DEFAULT gen_random_uuid(), workspace_id uuid NOT NULL REFERENCES workspaces(id), gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE, photo_id uuid NOT NULL REFERENCES photos(id) ON DELETE CASCADE, author text NOT NULL CONSTRAINT photo_comments_author_known CHECK (author IN ('client','photographer')), body text NOT NULL CONSTRAINT photo_comments_body_length CHECK (char_length(body) BETWEEN 1 AND 500), created_at timestamptz NOT NULL DEFAULT now(), edited_at timestamptz, deleted_at timestamptz)`, index on `(photo_id, created_at, id)`. Replace `notifications_kind_known` with the check including `'comment'` (as `0026_notification_opened.sql` does). Add trigger `photo_comments_record` `AFTER INSERT ... WHEN (NEW.author = 'client')` calling a new `notify_comment_activity()` that runs `record_notification(NEW.workspace_id, NEW.gallery_id, 'comment', 1)`. Comment the migration like its neighbours.
- [ ] **Step 4: Implement `comments.rs` and `NotificationKind::Comment`.** Each write is one SQL statement or one transaction following `set_favorite`'s style: lock the gallery row `FOR SHARE`, check the photo is a proof of `location`, and decide `Delivered` from `galleries.delivered_at`. `edit` and `delete` match on `id`, `workspace_id`, `gallery_id` and `author`; `delete` uses `COALESCE(deleted_at, now())` so repeats keep the first time. `thread` blanks the body in SQL for client viewers (`CASE WHEN deleted_at IS NOT NULL THEN NULL ELSE body END`), so the text never reaches the API process.
- [ ] **Step 5: Run** `cargo test -p pp-database` and `cargo clippy -p pp-database --all-targets`. Expected: PASS, no warnings.
- [ ] **Step 6: Commit** `feat: store comments on proof photos (#321)`.

---

### Task 2: Notification text for comments (API side)

**Files:**
- Modify: `crates/api/src/routes/notifications.rs` only if it enumerates kinds; otherwise none.

- [ ] **Step 1:** `grep -rn "NotificationKind::" crates/api crates/image-worker` and make every `match` handle `Comment`; add a test next to the existing notification route test asserting a client comment is listed with `"kind": "comment"` and `"count": 1`.
- [ ] **Step 2:** `cargo test -p pp-api notifications`. Expected: PASS.
- [ ] **Step 3: Commit** `feat: list comment notifications (#321)`. (Fold into Task 3's commit if nothing needed changing.)

---

### Task 3: Comment routes

**Files:**
- Create: `crates/api/src/routes/galleries/comments.rs`, `crates/api/src/comment_limiter.rs`
- Modify: `crates/api/src/config.rs` (+ its tests), `config.example.toml`, `crates/api/src/api.rs` (`ApiState` holds the limiter and config), `crates/api/src/lib.rs`, `crates/api/src/routes/galleries.rs` (register `comments::configure`; mirror how `shared` and `photos` are registered, before routes that would shadow them), `crates/api/src/extractors/client_access.rs`, `crates/api/src/error.rs`

**Interfaces:**
- Consumes: Task 1's `CommentRepository`, `CommentBody`, `CommentChange`, `CommentAuthor`, `CommentId`.
- Produces:
  - `CommentsConfig { min_interval: CommentInterval, max_per_photo: NonZeroU16 }` (`Args`/`ConfigArgs` like `LimiterConfig`; `CommentInterval = BoundedDuration<1_000, 60_000>`), as `ApiConfig::comments`.
  - `CommentLimiter::new(min_interval: Duration)` and `CommentLimiter::check(&self, gallery: GalleryId) -> Result<(), Duration>` (`Err` carries the time left; passing records the write; entries older than the interval are dropped on each call, so memory stays bounded to galleries written in the last interval; a `std::sync::Mutex` held briefly, never across `.await`).
  - `ErrorCode::ThreadFull` (409, wire name `thread_full`) with `ErrorResponse::thread_full()`, and `applied_comment` maps `CommentChange::Full` to it.
  - `ClientAccess::open_for_comments(&self) -> Result<PhotoLocation, ErrorResponse>` (409 `gallery_delivered` when `self.0.gallery.delivered`, else `self.0.location`); `ErrorResponse::applied_comment<T>(change: CommentChange<T>) -> Result<T, Self>` (404 / 409 like `ensure_favorite_applied`).
- Routes, same shapes under both prefixes. `P` is `/api/v1/galleries/shared/{token}` for clients and `/api/v1/galleries/{id}` for the photographer (`Authenticated`, location from `identity.workspace_id` and the gallery id):
  - `GET P/comments` → `200 [{ "photo_id", "count" }]`
  - `GET P/photos/{photo_id}/comments` → `200 [Comment]`, `404` when `thread` is `None`
  - `POST P/photos/{photo_id}/comments` with `{ "body": "…" }` → `201 Comment`
  - `PATCH P/comments/{comment_id}` with `{ "body": "…" }` → `200 Comment`
  - `DELETE P/comments/{comment_id}` → `204`
  - Client `POST`, `PATCH` and `DELETE` first call `CommentLimiter::check(gallery_id)` and answer `ErrorResponse::busy(left)` on `Err`; the photographer routes never do.
  - The viewer is `Client` for the client prefix and `Photographer` for the other; an invalid body is `ErrorResponse::bad_request()`.
- Update the module docs of `shared.rs`/`galleries.rs` route lists to mention the new routes.

- [ ] **Step 1: Write the failing tests** in `comments.rs` `mod tests`, reusing the fixtures of `shared.rs` tests (`shared_gallery`, a proof photo, `favorite_request`-style request builders; copy the helpers' approach, do not edit them):
  - `a_client_comments_and_reads_the_thread`: POST returns 201 with `author: "client"`; GET thread and GET counts show it; headers include `Cache-Control: no-store`.
  - `a_deleted_comment_never_leaks_its_text_to_the_client`: after DELETE, client GET returns `body: null` and `deleted_at` set; the photographer GET returns the original `body`.
  - `a_client_cannot_touch_a_reply_nor_the_photographer_a_client_comment`: PATCH and DELETE across authors give 404.
  - `delivered_galleries_close_every_client_comment_route`: all five client routes give 409 `gallery_delivered`; the photographer can still POST a reply (201).
  - `comments_refuse_bad_bodies_finals_and_unavailable_links`: empty and 501-char bodies give 422; a final photo, a foreign photo, an unknown, malformed, unshared and expired token give the usual 404; a photographer request for another workspace's gallery gives 404; unauthenticated photographer calls give 401.
  - `client_writes_are_paced_per_gallery`: with `min_interval` 2 s, a second client write right after the first gives 429 with a `Retry-After` header; a write in another gallery succeeds; after the interval (use a short configured interval such as 1 s and wait, or the limiter's own unit test with an injected clock-free `Duration`) it succeeds; photographer replies in a row all succeed.
  - `a_full_thread_refuses_client_posts`: with `max_per_photo = 2`, the third client post gives 409 `thread_full`; the photographer's reply still gives 201.
  - `a_client_comment_reaches_the_bell`: after a client POST, `GET /api/v1/notifications` lists one `comment` notification.
- [ ] **Step 2: Run** `cargo test -p pp-api comments`. Expected: compile failure, then FAIL.
- [ ] **Step 3: Implement** the handlers with `#[get]`/`#[post]`/`#[patch]`/`#[delete]` and register them in `configure`. Body type: `#[derive(Deserialize)] struct CommentInput { body: CommentBody }`; a rejected body (empty or over-long) must produce `bad_request()`, not the framework's default error (see how other JSON bodies map errors in this crate). Implement `open_for_comments` and `applied_comment` as the methods named above.
- [ ] **Step 3b:** Implement `CommentLimiter` with its own unit tests (first call passes, an immediate second returns `Err` with at most the interval, another gallery passes, expired entries are pruned), the config with defaults, tests in `config.rs` for defaults and for rejecting `0` or an interval over a minute (mirroring `invalid_limiter_settings_are_rejected_at_parse_time`), the `[comments]` block in `config.example.toml`, and the `thread_full` entry in the error-code tests (`error.rs` near the `GalleryDelivered` mapping) and in `web/src/lib/i18n/errors.ts` with its en/fr messages.
- [ ] **Step 4: Run** `cargo test -p pp-api`, `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check`. Expected: PASS.
- [ ] **Step 5: Commit** `feat: comment routes for clients and the photographer (#321)`.

---

### Task 4: Web client, thread component and viewer panel

**Files:**
- Create: `web/src/lib/gallery/comments.ts`, `web/src/lib/gallery/comments.test.ts`, `web/src/lib/components/CommentThread.svelte`, `web/src/lib/components/CommentThread.test.ts`
- Modify: `web/src/lib/gallery/schemas.ts` (comment schemas, add `'comment'` to the notification kind picklist), `web/src/lib/components/ProofViewer.svelte`, `web/src/lib/components/ProofViewer.test.ts`, `web/messages/en.json`, `web/messages/fr.json`

**Interfaces:**
- Produces:
  - `commentSchema` / `type Comment = { id: string; photo_id: string; author: 'client' | 'photographer'; body: string | null; created_at: string; edited_at: string | null; deleted_at: string | null }` and `commentCountsSchema` (array of `{ photo_id, count }`).
  - `class CommentsClient { constructor(base: string, fetcher?: typeof fetch); counts(): Promise<Map<string, number>>; thread(photoId: string): Promise<Comment[]>; add(photoId: string, body: string): Promise<Comment>; edit(commentId: string, body: string): Promise<Comment>; remove(commentId: string): Promise<void> }`. `base` is `/api/v1/galleries/shared/{token}` or `/api/v1/galleries/{id}`; paths are those of Task 3. Failures throw `GalleryApiError` like `api.ts` (reuse `apiErrorMessage`).
  - `CommentThread.svelte` props: `client: CommentsClient`, `photoId: string`, `viewer: 'client' | 'photographer'`, `onchange?: () => void`. It loads the thread, lists comments oldest first, shows "edited" and the author ("You" for the viewer's own side, "Client"/"Photographer" for the other), a deleted client-side comment as "Comment deleted", a photographer-side deleted comment as struck-through text, offers edit and delete only on comments the viewer may change (client: any client comment; photographer: her own), and has a textarea (max 500 chars, line breaks kept) with a send button disabled while empty. Errors show the API message in an `role="alert"` line.
  - `ProofViewer` new optional props `thread?: Snippet<[Proof]>` and `commentCount?: (id: string) => number`. When `thread` is given, the top bar shows a comments button (icon `MessageSquare`, count) with `aria-expanded`; it toggles a panel (side panel at `min-width: 701px`, bottom sheet below) rendered with `{@render thread(proof)}`. The panel starts closed, closes when `proof.id` changes, and Escape closes it before it closes the viewer.

- [ ] **Step 1: Write the failing tests.**
  - `comments.test.ts`: with a stub `fetcher`, `counts()` maps the array to a `Map`; `add` POSTs `{ body }` to `${base}/photos/p1/comments`; `edit` PATCHes and `remove` DELETEs `${base}/comments/c1`; a 409 response with `{"code":"gallery_delivered"}` throws `GalleryApiError` with that `code`.
  - `CommentThread.test.ts`: lists comments; a deleted comment shows "Comment deleted" for `viewer: 'client'` and struck-through text (an element with `line-through`) for `photographer`; the send button is disabled for blank text; sending calls `add` and shows the new comment and calls `onchange`; edit and delete controls follow the permission rules above; an API failure shows the alert.
  - `ProofViewer.test.ts`: without `thread` no comments button exists; with it the panel is absent until the button is clicked, the button shows the count, clicking again closes it, moving to the next photo closes it, and Escape closes the panel but not the viewer.
- [ ] **Step 2: Run** `pnpm --dir web test -- comments CommentThread ProofViewer`. Expected: FAIL.
- [ ] **Step 3: Implement** the files above. Add every string (comments button label, placeholder, send, edit, delete, save, cancel, "edited", "Comment deleted", "You", "Client", "Photographer", length hint) to both message files, then `pnpm --dir web paraglide`. Style from `docs/designs/ui/DESIGN.md` and the dark viewer tokens already used in `ProofViewer`; use a plain `<textarea>` styled like the `ui/Input` primitive.
- [ ] **Step 4: Run** `pnpm --dir web verify`. Expected: PASS with zero warnings.
- [ ] **Step 5: Commit** `feat: comment thread in the proof viewer (#321)`.

---

### Task 5: Client and photographer screens

**Files:**
- Modify: `web/src/lib/components/ClientGallery.svelte` (+ `ClientGallery.test.ts`), `web/src/lib/components/PhotosPanel.svelte` and `FavoritesPanel.svelte` (+ their tests), `web/src/lib/components/ProofGrid.svelte` / `PhotoTile.svelte` / `ProofThumbnail.svelte` (badge, as the grid structure requires), `web/src/lib/gallery/notifications.svelte.ts` (+ test), messages en/fr.

**Interfaces:**
- Consumes: `CommentsClient`, `CommentThread`, `ProofViewer`'s `thread` and `commentCount` props, `getLiveEvents().onNotification`.

- [ ] **Step 1: Write the failing tests.**
  - `ClientGallery.test.ts`: with a `token`, proofs and not delivered, the viewer offers comments and posting a comment calls `POST …/photos/<id>/comments`; counts load once and a small count badge shows on tiles that have comments; in `preview` mode (no token) and when `delivered`, no comments button and no badge.
  - `PhotosPanel.test.ts` / `FavoritesPanel.test.ts`: on the proofs view the viewer offers the photographer's thread using `/api/v1/galleries/<id>/…`; on the finals view it does not; a `notification` event for this gallery refetches the counts and the open thread.
  - `notifications` test: a `comment` notification renders "N new comments" (plural, with `*` catch-all) and a "Comments" badge, in both languages.
- [ ] **Step 2: Run** `pnpm --dir web test`. Expected: FAIL on the new tests only.
- [ ] **Step 3: Implement.** Build a `CommentsClient` per gallery (client base from the token, photographer base from the gallery id); keep counts in component state, refresh them on `onchange` and on notification events for the gallery; pass `thread`/`commentCount` to `ProofViewer`; show the badge on tiles. Extend `notifications.svelte.ts` text and badge helpers and the messages. The client page never subscribes to events.
- [ ] **Step 4: Run** `pnpm --dir web verify`. Expected: PASS with zero warnings.
- [ ] **Step 5: Commit** `feat: show and reply to photo comments (#321)`.

---

### Task 6: Spec and README

**Files:**
- Modify: `docs/SPECIFICATIONS.md`, `README.md`

- [ ] **Step 1:** In `docs/SPECIFICATIONS.md` add `### FR-08a` "Photo comments" after FR-08 (summary of the design's goals, edit/delete/soft-delete rule, no client notification, proofs only and closed after delivery) and an acceptance criterion `AC-03a` referencing it; update the client journey line ("Choose and review") to mention comments. Maintainer approval of this change was given with the design.
- [ ] **Step 2:** In `README.md` (status paragraphs around the favorites and notifications text) mention the comment routes and the `comment` notification, keeping claims true to the code.
- [ ] **Step 3: Commit** `docs: photo comments in the specification and README (#321)`.

---

### Task 7: Final verification

- [ ] **Step 1:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (as listed in `crates/AGENTS.md`) and `pnpm --dir web verify`. Expected: all PASS. Report any that cannot run.
- [ ] **Step 2:** Request a fresh independent reviewer (correctness and compliance with the design), fix material findings, do a refactor pass, then open the PR for #321 and assign @alekece.
