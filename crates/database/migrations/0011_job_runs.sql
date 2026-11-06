-- Jobs are deleted once done, so the last success of a recurring kind is kept here, letting a
-- handler skip a run that comes too soon after the previous one, restarts included.
CREATE TABLE job_runs (
    kind text PRIMARY KEY,
    finished_at timestamptz NOT NULL
);
