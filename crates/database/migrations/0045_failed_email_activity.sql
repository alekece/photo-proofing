ALTER TABLE activity_events
    DROP CONSTRAINT activity_events_kind_known,
    ADD CONSTRAINT activity_events_kind_known CHECK
        (kind IN ('open', 'favorite', 'unfavorite', 'comment', 'download', 'email', 'email_failed')),
    ADD COLUMN failed_email_job uuid UNIQUE REFERENCES jobs(id) ON DELETE SET NULL;

-- A final lease expiry and an ordinary exhausted send are the same state transition.
CREATE FUNCTION record_failed_email() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    mail jsonb := NEW.payload->'payload';
    gallery galleries%ROWTYPE;
    recipient uuid;
BEGIN
    SELECT * INTO gallery FROM galleries
        WHERE id = (mail->>'gallery_id')::uuid AND workspace_id = (mail->>'workspace_id')::uuid
        FOR KEY SHARE;
    IF NOT FOUND THEN RETURN NULL; END IF;

    IF mail->'recipient'->>'kind' = 'contact' THEN
        SELECT id INTO recipient FROM contacts
            WHERE id = (mail->'recipient'->>'contact_id')::uuid AND workspace_id = gallery.workspace_id
            FOR KEY SHARE;
    END IF;

    INSERT INTO activity_events (workspace_id, gallery_id, contact_id, kind, count, failed_email_job)
        VALUES (gallery.workspace_id, gallery.id, recipient, 'email_failed', 1, NEW.id)
        ON CONFLICT (failed_email_job) DO UPDATE SET last_at = now();
    RETURN NULL;
EXCEPTION WHEN invalid_text_representation THEN
    -- A malformed job must not prevent the worker from claiming other work.
    RETURN NULL;
END;
$$;

CREATE TRIGGER jobs_record_failed_email AFTER UPDATE OF state ON jobs
    FOR EACH ROW WHEN (NEW.state = 'failed' AND OLD.state <> 'failed' AND NEW.payload->>'kind' = 'send_email')
    EXECUTE FUNCTION record_failed_email();

CREATE VIEW retryable_email_activity AS
SELECT e.id AS event_id, e.workspace_id, e.gallery_id, e.contact_id, j.id AS job_id
FROM activity_events e
JOIN jobs j ON j.id = e.failed_email_job
JOIN galleries g ON g.id = e.gallery_id AND g.workspace_id = e.workspace_id
WHERE e.kind = 'email_failed' AND j.state = 'failed' AND j.payload->>'kind' = 'send_email'
  AND j.payload->'payload'->>'gallery_id' = g.id::text
  AND j.payload->'payload'->>'workspace_id' = g.workspace_id::text
  AND g.share_token IS NOT NULL AND g.archived_at IS NULL
  AND (g.expires_at IS NULL OR g.expires_at >= (now() AT TIME ZONE 'UTC')::date)
  AND j.payload->'payload'->>'stage' = CASE WHEN g.delivered_at IS NULL THEN 'proofing' ELSE 'finals' END
  AND (j.payload->'payload'->'recipient'->>'kind' = 'bcc'
       OR (j.payload->'payload'->'recipient'->>'kind' = 'contact'
           AND j.payload->'payload'->'recipient'->>'contact_id' = e.contact_id::text
           AND EXISTS (SELECT FROM gallery_contacts gc WHERE gc.gallery_id = g.id AND gc.contact_id = e.contact_id)));
