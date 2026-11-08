# Live photo events over SSE

Issue: #80. Requirements: TC-05, NF-03, AC-08; [ADR 0001](../adr/0001-technology-baseline.md)
"Browser updates". Builds on the [job queue](2026-10-31-job-queue-design.md) (#97) and
[proof derivatives](2026-11-04-proof-derivatives-design.md) (#99) designs. First layer of a stack
with the Photos tab upload (#102) and bulk photo deletion (#146).

## Intent

A photographer watching a gallery sees each photo move from waiting to processing to ready (or
failed, or deleted) as it happens, in every open tab, without reloading. A dropped connection
recovers without missing anything.

## Goals

- One photographer-wide stream, `GET /api/v1/events`, carrying small photo events, designed so
  later slices (notifications, ZIP readiness) add event types to the same stream.
- The worker's and the API's photo changes reach the stream with no new service.
- Every (re)connection starts with a signal to refetch, so missed changes are recovered.
- The stream is behind the photographer session, carries only that workspace's events, and ends on
  logout or session expiry.
- Bounded memory: one bounded buffer shared by all streams, so a client that stops reading only
  lags and is told to resync.

## Non-goals

- Any client (shared link) stream: the shared page has none.
- Notification, ZIP and gallery-setting events: their slices add them.
- Durable event history or replay by event id: PostgreSQL stays authoritative.

## Anti-goals

- A stream that renews the session (the PRD says SSE does not count as activity).
- Another workspace's ids, file names or failure reasons on the stream, or in logs.
- An event that can be lost silently: a gap must always end in a refetch.

## Approaches

1. **PostgreSQL `LISTEN/NOTIFY` from a trigger on `photos` (chosen).** Every insert, status change
   and delete notifies on commit, whichever process made it (upload, retry, worker, deletion). The
   API holds one listening connection and fans out in memory. No writer can forget to notify.
   Cost: a trigger to maintain, and NOTIFY is lost if the API's listener is disconnected, which
   the design turns into a refetch.
2. **Explicit `pg_notify` calls in each repository function.** Same transport, easier to read in
   Rust, but every new writer must remember it; a missed call is a silent bug.
3. **API polls the `photos` table per open stream.** No trigger, but repeated queries per tab
   while idle, which the ADR rules out as the default.

## Decisions

Decided with the product owner:

- One small event per photo change, carrying the photo id and its status, so the page updates
  without refetching the list.
- Events that keep the interface lively without making the server busy: no progress percentages,
  no per-tab polling.

My calls (to check while reading):

- **Trigger.** An `AFTER INSERT OR UPDATE OF status OR DELETE` row trigger on `photos` (updates
  only when the status actually changes, `OLD.status IS DISTINCT FROM NEW.status`) calls
  `pg_notify('photo_events', json)` with `workspace_id`, `gallery_id`, `photo_id` and the new
  `status` (`deleted` on delete). The payload never carries names or failure reasons, so it stays
  far below NOTIFY's 8000-byte limit. NOTIFY is transactional: nothing is sent for a rolled-back
  change. A gallery deletion cascades to its photos and so notifies each of them, which is the
  right outcome for an open page.
- **Listener.** One relay task per API process owns one `sqlx::postgres::PgListener` listening on
  every event channel (`photo_events`, `favorite_events`, `export_events`). It decodes each
  notification by its channel name and publishes it on a bounded `tokio::sync::broadcast`
  channel. A notification it cannot decode becomes a `resync` for everyone, as does every
  (re)connection, because notifications sent meanwhile are gone. If the connection cannot be
  reopened, the task backs off and starts again.
- **Stream route.** `GET /api/v1/events` uses `Authenticated` (read-only, never renews), then
  answers with `actix_web_lab::sse`. All actix-web-lab use stays in one module
  (`crates/api/src/routes/events.rs`), so its evolving API is contained. Each connection subscribes to
  the broadcast channel and forwards only events of its workspace.
- **Events.**
  - `resync` (no data) is always the first event, and is sent again after a lag or a listener
    reconnect. The page refetches what it shows.
  - `photo` with the notification payload as is, `{"workspace_id", "gallery_id", "photo_id",
    "status"}`, `status` being `waiting`, `processing`, `ready`, `failed` or `deleted`. The page
    ignores `workspace_id`, which is always its own. A failed photo's reason is read from the list when the page
    needs it, so reasons never travel on the stream.
- **Slow clients.** The broadcast channel has a fixed capacity; a connection that falls behind gets
  `Lagged`, which it turns into one `resync` and carries on. A client that stops reading costs
  nothing more: actix stops polling its stream, and the shared buffer does not grow. The
  connection is dropped when the socket closes or its session ends.
- **Session end.** Each connection re-checks its session on a fixed interval (alongside the
  keep-alive), with the same `AuthRepository::authenticate` call as `Authenticated`, and ends the
  stream once the session is gone, expired or logged out. A logged-out tab may therefore receive
  events for at most that interval; they are ids and statuses of its own workspace only.
- **Limits.** No cap on open streams: the MVP has one account, and all streams share the
  broadcast buffer. Keep-alive comment every ~15 s, under typical proxy idle timeouts; Caddy
  already flushes `text/event-stream`.
- **Logging.** The `/events` route logs connection open/close with the workspace id only, never
  event payloads.
- **Web.** A dashboard layout (`routes/dashboard/+layout.svelte`) opens one `EventSource` for the
  whole signed-in area and exposes the events through a small store (`lib/events.ts`).
  Pages subscribe to the events they care about; a `resync` makes them refetch. `EventSource`
  retries by itself only after a network drop: a 401 or 503 answer closes it for good and hides the
  status. So when it closes, the store asks `GET /api/v1/auth/status`; signed out sends the page to
  login, otherwise it reopens the stream after a growing delay. A stream the server ended at
  session expiry follows the same path.
- **Consumer.** This layer ships the stream, the layout and the store with their tests; the first
  page that listens is the Photos tab in #102.

## Left to the builder

- Numeric defaults: channel capacity, session re-check interval, keep-alive, reconnect backoff.
- Exact trigger SQL and migration name; whether the listener task restarts itself or relies on
  `PgListener`'s own reconnect, as long as a reconnect yields a `resync`.
- How tests drive the stream (actix test server reading the body).

## Verification

Tests against PostgreSQL: a photo insert, status change and delete each produce one `photo` event
on a connected stream of the owning workspace and none on another workspace's stream; the first
event of every connection is `resync`; a lagging subscriber gets a `resync`; a logged-out session's
stream ends within the re-check interval; the route needs a session and does not extend it; the
relay holds one listener connection for all channels; a listener reconnect produces a `resync`. Web tests cover the store dispatching events
and the refetch on `resync`.
