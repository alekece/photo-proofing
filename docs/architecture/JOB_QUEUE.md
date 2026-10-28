# PostgreSQL Job Queue

## Purpose

Some tasks should not run inside an API request because they are slow or retryable: image inspection, proof generation, watermark regeneration, final rendition generation, selected-source ZIP exports, delivery ZIP exports, retention tasks, and cache cleanup.

The API writes a durable job record to PostgreSQL and returns promptly. A separate Rust worker process claims and performs jobs. The photographer dashboard reads job state and displays `processing`, `ready`, `failed`, or a retry action.

## Minimum job record

A job needs at least:

- ID and job type.
- Gallery ID and related photo, selection snapshot, or delivery manifest ID.
- Input version or idempotency key.
- State: pending, leased, completed, retryable failure, or terminal failure.
- Attempt count and maximum attempts.
- Scheduled time, lease expiration, started, completed, and updated timestamps.
- A safe error code and photographer-readable message without credentials or private URLs.
- Output asset ID or export ID once complete.

## Claiming and reliability

Workers claim a small batch transactionally with PostgreSQL row locking such as `FOR UPDATE SKIP LOCKED`. A claim creates a time-limited lease. Workers extend the lease with a heartbeat while processing.

Every job must be idempotent. Retrying proof generation for the same source and configuration must not create two current proofs or publish a partial file. Use uniqueness constraints on derivative configuration and verify an output object before marking the job complete.

On crash or lease expiry, another worker may safely retry. Failed jobs use exponential backoff and end in a terminal failed state after a defined limit. A failed export can be retried by the photographer.

## ZIP exports

ZIP creation is asynchronous. It operates only on an immutable selection snapshot or delivery manifest. The worker rejects unsafe archive path names, enforces file and total-size limits, records an audit event, and creates a short-lived private export grant. The source or delivery ZIP is deleted after its defined retention interval.

The selected-source ZIP is photographer-only and should require a recent authenticated photographer session. A delivery ZIP is available only to an authorised client during Delivery when enabled by the photographer.

## Required tests

- Two workers cannot complete the same logical derivative twice.
- Worker termination mid-job leaves the job recoverable.
- Storage failure does not publish a partial or unauthorised object.
- Duplicate enqueue returns the existing valid job where appropriate.
- A ZIP contains exactly the frozen manifest assets, no more and no fewer.
- Client sessions cannot obtain a selected-source ZIP.
- Queue errors do not expose credentials, storage paths, tokens, or client image URLs.