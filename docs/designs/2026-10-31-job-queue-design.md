# Durable job queue in the image worker

Issue: #97 (sub-issue of #66). Requirements: NF-02, NF-04, TC-04, AC-06; [ADR 0001](../adr/0001-technology-baseline.md) "Background work".

## Intent

Processing must continue after the photographer leaves the page and survive API or worker
restarts and crashes, without losing accepted work or running it twice at the same time. The
worker is idle today and has no database access; this slice makes it a reliable job runner.

## Goals

- The worker connects to PostgreSQL, applies migrations before it queries, and claims jobs from a
  durable table.
- Accepted jobs survive restarts; interrupted jobs become eligible again; concurrent workers
  never run the same job at once.
- Retries are bounded with backoff. Exhausted jobs stay visible as failed and can be retried
  manually.
- Worker concurrency and per-job resource use are bounded.

## Non-goals

- Image processing, the `photos` table, and upload (#98, #99).
- `LISTEN/NOTIFY` or any push signal; progress and live updates belong to #80.
- A job-status API or UI; #99 and #102 expose failures to the photographer.
- A separate queue service (TC-04) or priorities and scheduling beyond a retry time.

## Anti-goals

- A job that crashes the worker being retried forever.
- A stale job recreating the files or rows of a deleted gallery.

## Approaches

1. **Generic `jobs` table, `kind` plus a typed payload (chosen).** One mechanism for all
   background work; handlers are registered by kind. Photo processing is the first kind (#99), a
   ZIP build can be another. Cost: no foreign key to the subject, so deletion safety is enforced
   by the handler and by gallery deletion instead of by a cascade.
2. **`photo_jobs` table with a foreign key to photos.** Deletion safety comes from a cascade, but
   it needs a `photos` table here and welds the queue to photos. Rejected by the product owner in
   favour of a generic queue.
3. **Queue columns on `photos`.** One row, no sync, but the same coupling and the same ordering
   problem. Rejected for the same reason.

## Decisions

Decided with the product owner:

- A generic queue, `jobs`, with `kind` and a payload.
- Photos are the only job kind for now; building ZIPs is a possible later kind.
- Retry backoff grows but with an upper cap that is not very high.
- A fixed poll interval (a setting), so an empty queue is not polled frantically but also never
  goes quiet for long.
- `kind` and `state` are enums in Rust and in PostgreSQL (`job_kind`, `job_state`), not text,
  like `gallery_layout`.
- A lease: a claim sets `locked_until` and the worker renews it while a job runs, so a crashed
  worker's job is reclaimed automatically once the lease passes.

My calls (to check while reading):

- **Table.** `jobs(id, kind, payload jsonb, state, attempts, max_attempts, run_at, locked_until,
  last_error, created_at)`. `kind` and `state` are PostgreSQL enums mirrored by Rust enums. States: queued, running, failed.
  Adding a kind is a migration; PostgreSQL cannot use a new enum value in the transaction that
  adds it, so it goes in its own migration.
- **Claim.** One `UPDATE ... WHERE id = (SELECT ... FOR UPDATE SKIP LOCKED) RETURNING`. A job is
  eligible when it is queued and due, or running with an expired lease. `attempts` increments at
  claim, so a job that keeps killing the worker also reaches the cap and becomes failed.
- **Outcome.** Success deletes the row, since the domain row (the photo) carries the user-visible
  status. A recoverable error requeues with exponential backoff capped at a small maximum. At the
  attempt cap the job becomes failed with its last error and stays until a manual retry resets
  it.
- **Handlers.** The worker registers one handler per kind; the payload is deserialised into the
  handler's type. An unreadable payload fails the job visibly; an unknown kind cannot be stored.
- **Repository.** The queue is a `Repository` in `pp-database` (`enqueue`, `claim`, `renew`,
  `complete`, `fail`, `retry`), consistent with the existing data-access pattern.
- **Migrations.** The worker runs the same migrator as the API at startup. SQLx serialises it
  with an advisory lock, so both processes may run it. Compose gives the worker its database URL.
- **Settings.** Typed `Duration`s for the poll interval, lease and backoff cap, and a bounded
  concurrency number, with defaults; the database setting reuses `DatabaseConfig`.
- **Shutdown.** On SIGTERM the worker stops claiming and gives running jobs a bounded time to
  finish; anything left is recovered by lease expiry.
- **Idempotency and deletion contract.** A handler must be safe to run twice. It writes outputs
  through the atomic storage layer (#96) to deterministic keys, and must confirm its subject still
  exists in the same transaction that records the result, so late work cannot recreate deleted
  content. Gallery deletion (#73) removes its pending jobs.

## Left to the builder

- Numeric defaults (poll interval, lease length, attempts, backoff base and cap, concurrency).
- Heartbeat mechanics and how the lease relates to the slowest expected job.

## Verification

Tests against PostgreSQL: concurrent claimers never receive the same job; an expired lease is
reclaimed and a live one is not; backoff grows to its cap; the attempt cap produces a visible
failed job that a manual retry requeues; a crash-looping job stops at the cap; shutdown during a
job leaves it recoverable. `docker compose up` shows the worker applying migrations and idling on
an empty queue.
