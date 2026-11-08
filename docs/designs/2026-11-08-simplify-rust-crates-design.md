# Simplify the Rust crates

Issue: #234.

## Intent

Four pieces of machinery cost more than they give. Removing them keeps behaviour and shrinks
what a reader must understand.

## Changes

1. **Unused notification module.** `pp-core::notifications` had no users beyond `PhotoId`,
   which now lives in `pp-database` next to the photo code. The module and core's `uuid`
   dependency are removed.
2. **One live-event listener.** One `PgListener` listens on all three trigger channels and a
   single relay task publishes to the in-process hub. `Channel`, the generic `Listener<T>`,
   stream slots, the stall watchdog and their configuration are removed. Slow clients are
   already handled by lag-to-resync, and the pool is bounded by request concurrency.
3. **Shared job deletion.** Gallery and photo deletion share one `delete_photo_jobs_in`.
4. **Failed sign-in middleware.** `FailedAttempts::limit` is one function wrapped by
   `actix_web::middleware::from_fn`; it replaces the `Transform`/`Service` pair and the
   `Attempt` extractor.

## Approaches considered

- Keep one listener per event type: simple types, but three connections and three reconnect
  loops. Rejected.
- Keep stream caps and the watchdog as defence in depth: they guard a threat the single
  listener and bounded pool already cover, at high complexity. Rejected.
- Keep the extractor for the over-limit check: duplicates what the middleware can do first.
  Rejected.

## Behaviour preserved

Each event reaches only its workspace; reconnect or lag produces a resync; a revoked session
ends the stream; failed sign-in limiting is unchanged. Trigger channels and the SSE event
format are unchanged.
