# Image worker: runtime-owned job outcomes and timer-based maintenance

Issue: https://github.com/alekece/photo-proofing/issues/233

## Intent

Handlers had grown into mini-runtimes: each finished its own job, the photo handler detached its publication from lease cancellation, and the orphan sweep was a job that remembered its last run in `job_runs` and also ran export expiry. The worker should have one place that owns job outcomes, and housekeeping should be a timer, not a job.

## Design

### Outcomes

A handler does the work and returns a typed **output**. It never sees the claimed job. The runtime then calls the kind's **finalizer** with the job and the output; the finalizer records the domain result and deletes the job in one transaction, and reports whether the claim was still current.

- `ProcessPhoto` returns `Skipped` (photo deleted, finished, or without a watermark), `Failed(reason)` (unreadable or too large) or `Rendered` (derivatives staged as temporary files). Its finalizer completes the job, fails the photo, or publishes: it locks the job and photo, checks the watermark stamp, renames the staged files, marks the photo ready and deletes the job.
- `ZipExport` returns `Gone`, `Failed(reason)` or `Built`. Its finalizer completes the job, fails the export, or marks the export ready and deletes the job.
- Cleanup handlers return `()`; the generic `CompleteJob` finalizer deletes the job.

Retry and abandon stay with the runtime: `HandlerError::Failed` backs off and retries, `HandlerError::Payload` abandons the job.

### Approaches considered

1. **Handlers keep finishing their own jobs.** No change, but every handler repeats lease and ownership handling and the detached publication stays.
2. **One generic outcome enum for all kinds.** Simple registry, but domain results (photo ready, export failed) would leak into one shared type.
3. **Output per handler plus a finalizer per kind (chosen).** The runtime owns ordering and error handling; each kind owns only its domain transaction. SQLx stays in the `database` crate, which exposes the composite operations the finalizers call.

## Invariants

- **Atomic.** A result and the deletion of its job commit together, or neither does.
- **Lease.** Finalizers re-check ownership under a row lock. Handling is raced against lease renewal as before; finalization is not, so a lost lease cannot cancel it half-way. A stale claim finalizes as "not current" and changes nothing.
- **Watermark.** Rendering reads the stamp after the photo is marked processing; publication requires the stamp to be unchanged, so an outdated proof is never shown. A change after that queues another job.
- **Deleted photos.** If the photo is gone when the finalizer runs, the staged files are discarded and any derivatives already written are removed. If the process is stopped past its grace period during finalization, the transaction rolls back and the job is retried after its lease expires; files left for a photo deleted in the meantime are removed by the maintenance sweep once past the grace period.

## Maintenance timer

The worker runs one maintenance task beside the jobs: expire exports, sweep orphaned photo files, sweep unclaimed archives. Export expiry is a step of maintenance and is no longer tied to export builds.

- It takes a permit from the same concurrency slots as jobs, so housekeeping never exceeds the worker's concurrency.
- It is a single task: a run never overlaps the next. Missed ticks are skipped, not replayed.
- The first run happens when the worker starts, replacing the old API startup sweep. The interval is the existing `sweep_interval` setting (`PP_CLEANUP_SWEEP_INTERVAL`).
- A failed step is logged and the run continues.
- It stops on shutdown, including while waiting for a slot.
- **Single worker.** Two workers would run maintenance concurrently. This is harmless (every step is idempotent and each orphan is re-checked against the database) but wasteful. If workers are scaled out, guard the run with a Postgres advisory lock.

## Migration

`0018_drop_job_runs.sql` deletes queued `sweep_orphans` jobs and drops `job_runs`. `JobKind::SweepOrphans` and its payload are removed.

## Out of scope

Versioned derivative filenames, and removing the handler-side cleanup of derivatives for deleted photos.
