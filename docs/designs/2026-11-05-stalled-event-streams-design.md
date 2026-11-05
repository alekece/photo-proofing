# Close live-event streams whose client stopped reading

Issue: #155. Builds on the [live photo events](2026-11-04-live-photo-events-design.md) design (#80).

## Intent

A browser that stops reading its live-events stream can't keep a stream slot forever and lock
other photographers out of live updates.

## Goals

- A stream whose client hasn't accepted data for a bounded time ends, and its slot is freed at
  once.
- Healthy idle streams, which only receive keep-alives, are unaffected.
- No new configuration: the bound derives from the existing `PP_EVENTS_KEEP_ALIVE`.

## Non-goals

- Closing the TCP connection itself. The protected resource is the stream slot (and its broadcast
  receiver). The operating system ends a dead connection on its own.
- Socket-level options such as `TCP_USER_TIMEOUT`: Linux-only, hard to test, and they affect
  every route.

## How a stall shows

actix polls a response body only while it can write. When the client stops reading, the socket
and actix's write buffer fill, and actix stops polling the stream. A healthy idle stream is still
polled at least once per keep-alive interval, because the keep-alive timer wakes it. So "not
polled for several keep-alive intervals" means the client isn't accepting data.

## Approach

- The stream records when it was last polled.
- A small watchdog task, spawned with the stream and stopped when the stream is dropped, checks
  every keep-alive interval.
- If the stream hasn't been polled for 3 keep-alive intervals (45 s with the default), the
  watchdog:
  - releases the slot and the broadcast receiver;
  - logs that the stream stalled;
  - leaves the stream to end on its next poll, if one ever comes.
- The slot and receiver move behind a shared handle that either the stream or the watchdog can
  release, exactly once.

## Testing

An API test with one slot and a short keep-alive:
- opens a stream, reads the first event, then stops reading;
- after the bound, a second stream for the same workspace is accepted;
- the stalled stream's body then ends.

A second test keeps reading an idle stream past the bound and checks that it still receives
keep-alives and holds its slot.
