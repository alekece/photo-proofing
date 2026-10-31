# Live photo updates over SSE

Issue: #80. Requirements: FR-05, FR-06, NF-03, TC-05; builds on the
[photo upload](2026-10-31-photo-upload-design.md) (#98), [photo processing](2026-10-31-photo-processing-design.md)
(#99) and [photos tab](2026-10-31-photos-tab-design.md) (#102) designs and
[ADR 0001](../../adr/0001-technology-baseline.md).

## Intent

After a file has been uploaded, the photographer watches each photo move through waiting,
processing and ready (or failed, with its reason) without reloading, and recovers cleanly after a
dropped connection or a restart. This replaces the "refresh on return" workaround of #102.

## Goals

- One photographer-wide SSE stream per browser tab, opened after login, carrying typed events
  tagged with their gallery. #80 delivers the photo events: the step of every photo.
- A reconnect restores the authoritative state; nothing depends on missed events.
- Bounded buffers, dead or slow connections cleaned up, nothing sensitive in the stream or logs.

## Non-goals

- Notification and ZIP-readiness events. The stream is designed to carry them, but the slices
  that own them add their event types and change detection.
- A client-side stream: the shared client page loads its photos when it opens and has no live
  updates. If a photo was removed meanwhile, the next action on it fails with "not found"; the
  client page then shows a message that the photographer is modifying the gallery and asks the
  client to refresh. That message belongs to the favorites slice (#67), not here.
- Browser polling, WebSockets, an event broker, `LISTEN/NOTIFY`, an event history, `Last-Event-ID`.
- Upload transfer progress (the browser measures it itself, see #102).

## Approaches

- **A. `LISTEN/NOTIFY`.** Instant, but needs a dedicated listener connection, reconnection and
  fan-out code across two processes.
- **B. API-side check of the database on a timer (chosen).** One shared loop reads what changed
  and fans out to open streams. No extra connection, no cross-process signalling; updates arrive
  about a second late, which is fine for processing. Switching to A later only changes the
  trigger.
- **C. Browser polling.** Ruled out by the ADR.

Per-photo streams were rejected: browsers allow about six connections per site over HTTP/1.1, so
a 100-file upload would starve other requests. One stream per tab carries every gallery's events.

## Design

- **Route.** `GET /api/v1/events`, behind the photographer session. It only ever delivers events
  for the session's workspace. No client route.
- **Events.** Each connection starts with a `snapshot` event holding the current status of the photos that
  are not yet ready or have failed (gallery id, photo id, status, failure reason); ready photos
  are loaded by the page itself, which refetches its photo list on every (re)connect so that
  nothing becomes ready unseen in between. It then sends a `photo` event per change. Every event has a type and a gallery id so the
  frontend can route it; later slices add types such as notifications and ZIP readiness.
  Payloads carry the step, so the tile can display it directly. They never contain file paths or
  other galleries' data.
- **Why a snapshot.** PostgreSQL is authoritative and SSE is not a history (ADR). A reconnect
  or a restart simply starts a new snapshot, which also recovers anything missed.
- **Change detection.** `photos` gets an `updated_at`, set by every status change. One loop in
  the API runs every 1 to 2 seconds, only while at least one stream is open, reads the photos
  changed since its last pass for the workspaces that have streams, and fans out. The interval is
  a named constant.
- **Ending a stream.** It ends when the session expires or is logged out, (a deleted gallery just stops producing events). The browser reconnects and, if no longer allowed, gets an
  error and stops.
- **Bounds.** Each connection has a small bounded buffer; a connection that falls behind is
  dropped (it reconnects and gets a fresh snapshot). Keep-alive comments are sent periodically so
  dead connections are noticed. The total number of open streams is a configurable limit, with
  `429` beyond it. Dropped connections are removed from the fan-out registry.
- **Logging.** Events and stream URLs contain no tokens; request logging stays off for
  secrets as elsewhere.
- **Library.** `actix-web-lab` SSE, kept in one module so its evolving API stays localized.
- **Frontend.** A small shared SSE client with reconnect and capped backoff. The #102 store
  merges events into its photo list and stops using the "refetch when a transfer finishes" step.

## Left to the builder

- Event naming and JSON shape, the exact intervals, buffer size and stream limit defaults.
- How the loop tracks per-workspace subscriptions and its "changed since" cursor.
- Where the SSE module lives in the API crate.

## Verification

Tests for: the snapshot on connect, a change arriving on an open stream, reconnect after a drop
giving the right state, a photographer without access being refused, stream ending on logout or
deletion, a slow consumer being dropped, the stream limit and cleanup after disconnect.
Needs #98, #99 and #102's store; the `photos.updated_at` column goes into the #98 migration or a
new one.
