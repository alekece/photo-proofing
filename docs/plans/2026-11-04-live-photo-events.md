# Live photo events: implementation plan

Design: [live photo events](../designs/2026-11-04-live-photo-events-design.md). Issue: #80.
Branch: `feat/80-live-photo-events`, bottom layer of the #80 → #102 → #146 stack.

Each task is test first: write the failing test, watch it fail, implement, watch it pass.

## 1. Trigger and listener (`pp-database`)

- Migration `0007_photo_events.sql`: `notify_photo_event()` plus two `AFTER` row triggers on
  `photos` (insert/delete; status update `WHEN OLD.status IS DISTINCT FROM NEW.status`), sending
  `{workspace_id, gallery_id, photo_id, status}` on channel `photo_events`, `status` = `deleted`
  on delete.
- `photo_events.rs`: `PhotoChange` (waiting, processing, ready, failed, deleted), `PhotoEvent`,
  `Notification::{Photo, Missed(MissedReason::{Reconnected, Undecodable})}`,
  `PhotoEventListener::next()` over `PgListener::try_recv` (`None` → `Missed(Reconnected)`).
  `Database::listen_photo_events()`. `WorkspaceId` gains `Hash`.
- Tests: insert, status change and delete each notify once; a same-status update and a
  rolled-back insert notify nothing; a garbage payload is `Missed(Undecodable)`.

## 2. Event hub and relay (`pp-api`, `events.rs`)

- `EventsConfig` (`[events]`): `keep_alive` (15 s, also the session re-check), `max_streams`
  (256), `max_streams_per_workspace` (32). Config tests for defaults and parsing.
- `EventHub`: bounded `broadcast` of `Broadcast::{Photo, Resync}`, stream counts behind a
  `std::sync::Mutex`, `subscribe(workspace)` returning `None` beyond either cap and a permit that
  frees the slot on drop.
- `relay_photo_events(database, hub)`: listen, publish each event, `Resync` after every
  (re)connect and every `Missed`; on error log, back off (250 ms doubling to 30 s), reconnect.
  Owned by `Api::run`, aborted when the server stops. Pool gets one more connection for it.
- Tests: a database change reaches a subscriber; terminating the listener's backend yields a
  `Resync`.

## 3. Stream route

- `GET /api/v1/events` (`Authenticated` + `SessionToken`), `Sse::from_infallible_stream` with
  keep-alive; stream state: first `resync`, then own-workspace `photo` events
  (`{gallery_id, photo_id, status}`), `Lagged` → `resync`, session re-check on each keep-alive
  tick ending the stream once the session is gone; a database error keeps the stream.
- Tests: 401 without session; first event `resync`; own event delivered, foreign skipped; lag →
  `resync`; logout ends the stream; caps answer 503 and free on disconnect; the session expiry is
  not extended.

## 4. Web store and dashboard layout

- `lib/events.svelte.ts`: `createEventStream({ open, authStatus, onSignedOut, schedule })` with
  `subscribe(type, handler)`; on `error` with `readyState === CLOSED` it checks
  `/api/v1/auth/status`: signed out → `onSignedOut`, otherwise reopen after 1 s doubling to 30 s
  (reset on `open`).
- `routes/dashboard/+layout.svelte`: opens one stream for the signed-in area, shares it through
  context (`getEvents()`), closes it on destroy.
- Tests (vitest, fake `EventSource`): dispatch by type, reopen with backoff after a non-auth
  close, sign-out path, close on destroy.

## 5. Docs and checks

- `config.example.toml` `[events]` section; README status line for live updates.
- `cargo +nightly fmt --all`, clippy `-D warnings`, `cargo test --workspace --all-features`,
  `cargo deny check` if available, `pnpm --dir web verify`.
- Fresh reviewer, refactor pass, PR (base `main`) assigned to @alekece.
