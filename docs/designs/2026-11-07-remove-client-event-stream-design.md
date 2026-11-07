# Remove the client event stream

Issue: [#174](https://github.com/alekece/photo-proofing/issues/174). Relates to #73.

## Why

`GET /api/v1/galleries/shared/{token}/events` only tells other browsers on the same link to refresh
their favorites. The PRD asks for no live client updates, and favorites are set idempotently, so the
stored selection is always right. The stream costs a second scope, a link recheck on every
keep-alive tick, a per-gallery slot owner and tests. Without it, expiry, archive and deletion (#73)
have no client stream to close.

## Goals

- Clients have no event stream; their pages use the normal API only.
- A client favorite change still reaches the photographer through `GET /api/v1/events`.
- Everything that exists only for the client stream is gone: code, tests, docs, page refresh logic.

## Non-goals

- No change to the workspace stream's behavior, events, caps or configuration.
- No change to favorite writes, the `favorites` notification or its trigger.
- No new client refresh mechanism (polling, refetch on focus). A stale browser keeps its optimistic
  selection until reload; accepted in the issue.
- No gallery dashboard UI changes: #175 (expired gallery prompt) edits that area in parallel, so
  this work stays out of it. #211 touches shared-link routes and is deliberately not run in
  parallel; this change edits `routes/galleries/shared.rs` (see below), so it should merge before
  #211 starts. `2026-11-06-gallery-lifecycle-design.md` already assumes #174 lands.

## Constraints

- AC-01 holds as written: clients cannot reach event routes. Nothing in `docs/SPECIFICATIONS.md`
  needs to change; "event routes" there then means the workspace route only.
- Per the repo guidance, the PRD is untouched without approval. This design needs none.

## Inventory

Removed:

- `crates/api/src/routes/events.rs`: `stream_shared_events`, `Scope::Link` (so `Scope` becomes a
  plain workspace struct), the link branch of `admits` and `is_valid`, the `Display` impl's link arm,
  the now-unused imports (`Path`, `ClientAccess`, `GalleryId`, `ShareToken`, `GalleryRepository`),
  the `Display` doc about the link token, and the module docs about shared links (lines 2 and 8-9). Tests: `a_link_stream_sends_only_its_own_gallery_favorites`,
  `an_unusable_link_gets_no_stream_and_a_revoked_one_loses_it`,
  `link_streams_do_not_use_the_workspace_slots`, plus helpers only they use (`shared_gallery`,
  `open_shared`). The 404 regression test (Decisions) goes in `routes/galleries/shared.rs`, with
  its own setup.
- `crates/api/src/routes/galleries/shared.rs`: the `.service(events::stream_shared_events)`
  registration, the now-unused `routes::events` import (clippy) and the module-doc line that
  says `GET /events` streams selection changes.
- `crates/api/src/events.rs`: `StreamOwner::Gallery` and `From<GalleryId>`. `StreamOwner` then has
  one variant; my call is to keep the enum out of this change's way by dropping it and keying
  `subscribe`, `acquire`, the slot map and its tests by `WorkspaceId`. Update the doc comments that
  mention shared-link streams and the "one shared link" cap wording.
- `web/src/lib/components/ClientGallery.svelte`: the `onMount` that opens an `EventSource`, plus the
  `onMount` import if unused. Tests: `refreshes from the link stream and closes it on leaving`, the
  `FakeEventSource` class, its setup/teardown and the "no stream" assertions (delivered and
  preview cases) in `ClientGallery.test.ts`; the `EventSource` stub and its comment in
  `routes.test.ts`. `dashboard/layout.test.ts` stubs `EventSource` for the workspace stream and stays.
- `web/src/lib/gallery/selection.svelte.ts`: `Selection.refresh` its two tests, its comment and any
  imports it leaves unused (`parseOrNull`, the favorites schema). The page is its only caller
  outside tests. `#saving` stays: `toggle` uses it. My call: remove it; dead code with tests is
  the leftover this issue exists to delete.
- Docs: `docs/designs/2026-11-05-client-selection-design.md` (see below). The older
  `2026-11-04-live-photo-events-design.md` already says the shared page has no stream and stays
  as is; the stalled-streams design speaks of streams generally and stays.

Stays:

- `GET /api/v1/events`, its session recheck, caps, stall watchdog, `resync` and `photo` /
  `favorites` events, and `Broadcast::Favorites`, which the workspace stream still forwards.
- The favorites notification, so the photographer's Favorites and Photos panels
  (`events.onFavorites`) refresh when a client toggles. The web `LiveEvents` code is unchanged.
- The shared routes, which still use `ClientAccess`.

## Design doc update

Edit `2026-11-05-client-selection-design.md` in place, in the implementation PR (as the issue and
the repo's "update in place, replace superseded decisions" rule say):

- Drop "Other browsers on the same link see changes live. After a connection loss they refresh…"
  and replace it with one line: a browser keeps its own selection until reload; the server list
  is what a fresh load shows.
- Line 30: keep the notification and photographer stream; remove "a per-link event stream for
  clients".
- Test list: drop "live refresh and reconnect refresh".

No other document mentions the client stream (`README.md`, ADRs and releases checked).

## Approaches

1. **Delete the stream and its refresh logic (recommended).** Remove everything in the inventory;
   collapse the link-only abstractions. Smallest runtime, matches the issue.
2. **Delete the route but keep the generalized `Scope`/`StreamOwner`.** Smaller diff, but leaves
   one-variant abstractions that only make sense for a second scope; invites later drift.
3. **Replace live refresh with refetch on tab focus.** Narrows staleness but is new behavior the
   PRD does not ask for, and adds a request pattern and tests. Rejected as scope creep.

## Decisions

- Approach 1. (my call)
- With the route removed, `GET /api/v1/galleries/shared/{token}/events` falls through to the
  normal not-found; a regression test asserts a valid link gets 404 there (status only; it is actix's empty default
  404) while the link's `/photos` route still answers 200, which pins AC-01. (my call)
- `Selection.refresh` is deleted if unused. (my call)
- No config or migration changes; `max_streams_per_workspace` keeps its name and meaning.
  (my call)

## Verification

`cargo` format, lint and tests per `crates/AGENTS.md`, and `pnpm --dir web verify`. Add a workspace-stream
test that publishes `Broadcast::Favorites` for its own and a foreign workspace and asserts only
the own `favorites` event arrives with its JSON; today only the deleted link test asserts that
wire shape. The existing hub test covers the database-to-broadcast step.

## Left to the builder

Exact import cleanup, the final wording of the edited design lines,
and the usual plan in `docs/plans/` once the design is approved (removed before merge).
