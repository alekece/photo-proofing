-- Adding a kind needs its own migration: PostgreSQL cannot use a new enum value in the
-- transaction that adds it.
CREATE TYPE job_kind AS ENUM ('process_photo');
CREATE TYPE job_state AS ENUM ('queued', 'running', 'failed');

CREATE TABLE jobs (
    id uuid PRIMARY KEY,
    kind job_kind NOT NULL,
    payload jsonb NOT NULL,
    state job_state NOT NULL DEFAULT 'queued',
    attempts integer NOT NULL DEFAULT 0 CONSTRAINT jobs_attempts_not_negative CHECK (attempts >= 0),
    max_attempts integer NOT NULL CONSTRAINT jobs_max_attempts_positive CHECK (max_attempts >= 1),
    run_at timestamptz NOT NULL DEFAULT now(),
    locked_until timestamptz,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT jobs_lease_iff_running CHECK ((state = 'running') = (locked_until IS NOT NULL))
);
-- Claims scan only work that can still run; failed jobs are kept for inspection.
CREATE INDEX jobs_claimable ON jobs(run_at) WHERE state <> 'failed';
