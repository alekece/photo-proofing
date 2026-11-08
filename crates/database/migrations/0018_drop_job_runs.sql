-- Maintenance runs on a timer in the worker, so the sweep job and its last-run record go.
DELETE FROM jobs WHERE payload ->> 'kind' = 'sweep_orphans';

DROP TABLE job_runs;
