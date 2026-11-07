ALTER TABLE photos ADD COLUMN clean_ready boolean NOT NULL DEFAULT false;
ALTER TABLE photos ADD COLUMN watermark_revision bigint;

-- Existing keys may contain marked pixels; only a new publication proves they are clean.
WITH reset AS (
    UPDATE photos SET status = 'waiting', failure_reason = NULL
    WHERE kind = 'proof' AND status IN ('ready', 'processing')
    RETURNING id, workspace_id
)
INSERT INTO jobs(id, payload, max_attempts)
SELECT gen_random_uuid(),
       jsonb_build_object('kind', 'process_photo',
                         'payload', jsonb_build_object('photo_id', reset.id)),
       COALESCE((
           SELECT j.max_attempts FROM jobs j
           JOIN photos p ON p.id = (j.payload->'payload'->>'photo_id')::uuid
           WHERE j.payload->>'kind' = 'process_photo'
             AND p.workspace_id = reset.workspace_id
           ORDER BY j.created_at DESC LIMIT 1
       ), 5)
FROM reset;
