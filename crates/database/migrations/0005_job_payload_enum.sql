ALTER TABLE jobs
    ALTER COLUMN payload TYPE jsonb
    USING jsonb_build_object('kind', kind::text, 'payload', payload);

ALTER TABLE jobs DROP COLUMN kind;
DROP TYPE job_kind;
