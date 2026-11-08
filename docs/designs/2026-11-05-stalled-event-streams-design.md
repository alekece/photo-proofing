# Close live-event streams whose client stopped reading (superseded)

Issue: #155. Superseded by [simplify Rust crates](2026-11-08-simplify-rust-crates-design.md)
(#234). Builds on the [live photo events](2026-11-04-live-photo-events-design.md) design (#80).

## Intent

A browser that stops reading its live-events stream must not lock other photographers out of
live updates.

## Outcome

The stream slots, the stall watchdog and their settings (`PP_EVENTS_STALL_INTERVALS`,
`PP_EVENTS_MAX_STREAMS`, `PP_EVENTS_MAX_STREAMS_PER_WORKSPACE`) were removed. Streams are no
longer counted, so there is no slot to hold: the MVP has one account, clients have no stream,
and all subscribers share one bounded broadcast buffer. A browser that stops reading makes
actix stop polling its stream; it only lags, is told to `resync` if it reads again, and ends with
its socket or its session.

Reconsider a per-workspace cap and a watchdog if the product gains many photographers per
deployment.
