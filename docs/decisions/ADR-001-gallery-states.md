# ADR-001: Explicit gallery state model

## Status

Accepted for MVP.

## Decision

A gallery follows `Draft -> Proofing -> Selection received -> Retouching -> Delivery -> Expired`. The photographer controls every transition. They may deliberately move backward with a warning where that can re-expose proof content.

## Consequences

The backend can make asset visibility deterministic. The client always knows whether it is choosing proofs, waiting for retouching, or receiving finals. State transitions and related activity are auditable.

The photographer creates an immutable selection snapshot when moving into Retouching. They create a delivery manifest when moving into Delivery. These avoid ambiguity when favourites or files change later.