# Close live-event streams whose client stopped reading

Issue: #155. Builds on the [live photo events](2026-11-04-live-photo-events-design.md) design (#80).

## Intent

A browser that stops reading its live-events stream can't keep a stream slot forever and lock
other photographers out of live updates.

## Goals

- A stream whose client hasn't accepted data for a bounded time ends, and its slot is freed at
  once.
- Healthy idle streams, which only receive keep-alives, are unaffected.
- The bound is a multiple of `PP_EVENTS_KEEP_ALIVE`, set by `PP_EVENTS_STALL_INTERVALS` (default 3, minimum 2, since a healthy idle stream is read once per period).

## Non-goals

- Closing the TCP connection itself. The protected resource is the stream slot. The operating system ends a dead connection on its own.
- Socket-level options such as `TCP_USER_TIMEOUT`: Linux-only, hard to test, and they affect
  every route.

## How a stall shows

actix polls a response body only while it can write. When the client stops reading, the socket
and actix's write buffer fill, and actix stops polling the stream. A healthy idle stream is still
polled at least once per keep-alive interval, because the keep-alive timer wakes it. So "not
polled for several keep-alive intervals" means the client isn't accepting data.

## Approach

- A small wrapper around the inner stream records the time on every `poll_next`, then delegates.
  The record must live there: the async `next()` stays suspended across polls, so a timestamp
  taken inside it would go stale and close healthy streams.
- A small watchdog task, spawned with the stream and stopped when the stream is dropped, checks
  every keep-alive interval.
- If the stream hasn't been polled for `PP_EVENTS_STALL_INTERVALS` keep-alive intervals (3, so 45 s, by default), the
  watchdog:
  - releases the slot;
  - logs that the stream stalled;
  - marks the stream ended, so it finishes on its next poll if one ever comes.
- Only the slot moves behind a shared handle that the stream or the watchdog releases, exactly
  once. The broadcast receiver stays with the stream: the pending `recv()` borrows it, and its
  memory is bounded by the hub capacity.

## Testing

An API test with one slot and a short keep-alive:
- opens a stream, reads the first event, then stops reading;
- after the bound, a second stream for the same workspace is accepted;
- the stalled stream's body then ends.

A second test keeps reading an idle stream past the bound and checks that it still receives
keep-alives and holds its slot.
