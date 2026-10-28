# ADR-003: PostgreSQL-backed asynchronous job queue

## Status

Accepted for MVP.

## Decision

The Actix Web API and a separate Rust worker share one codebase and PostgreSQL database. The API writes durable jobs to a PostgreSQL table. The worker claims jobs with transactional row locking and processes image and ZIP work asynchronously.

## Consequences

The MVP avoids a separate queue service while keeping slow image work out of request handling. The queue must implement leases, heartbeats, idempotency, retries, terminal failures, and safe output publication.