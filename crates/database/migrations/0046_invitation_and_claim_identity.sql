-- Neither a renewed gallery link nor a manual retry may inherit its predecessor's identity.
ALTER TABLE gallery_contacts ADD COLUMN link_id uuid NOT NULL DEFAULT gen_random_uuid() UNIQUE;
ALTER TABLE jobs ADD COLUMN claim_generation bigint NOT NULL DEFAULT 0;

-- Old personal invitations cannot prove which link was selected. Keep any undrained
-- jobs as visible failures instead of guessing from a contact ID or sending silently.
UPDATE jobs SET state = 'failed', locked_until = NULL, last_error = 'original gallery link unknown'
WHERE state <> 'failed' AND payload->>'kind' = 'send_email'
  AND payload #>> '{payload,recipient,kind}' = 'contact';

CREATE OR REPLACE VIEW retryable_email_activity AS
SELECT e.id AS event_id, e.workspace_id, e.gallery_id, e.contact_id, j.id AS job_id,
       CASE WHEN j.payload #>> '{payload,recipient,kind}' = 'contact'
            THEN (j.payload #>> '{payload,recipient,link_id}')::uuid END AS link_id
FROM activity_events e
JOIN jobs j ON j.id = e.failed_email_job
JOIN galleries g ON g.id = e.gallery_id AND g.workspace_id = e.workspace_id
WHERE e.kind = 'email_failed' AND j.state = 'failed' AND j.payload->>'kind' = 'send_email'
  AND j.payload #>> '{payload,gallery_id}' = g.id::text
  AND j.payload #>> '{payload,workspace_id}' = g.workspace_id::text
  AND g.share_token IS NOT NULL AND g.archived_at IS NULL
  AND (g.expires_at IS NULL OR g.expires_at >= (now() AT TIME ZONE 'UTC')::date)
  AND j.payload #>> '{payload,stage}' = CASE WHEN g.delivered_at IS NULL THEN 'proofing' ELSE 'finals' END
  AND (j.payload #>> '{payload,recipient,kind}' = 'bcc'
       OR (j.payload #>> '{payload,recipient,kind}' = 'contact'
           AND j.payload #>> '{payload,recipient,contact_id}' = e.contact_id::text
           AND EXISTS (SELECT FROM gallery_contacts gc WHERE gc.gallery_id = g.id AND gc.contact_id = e.contact_id
                       AND gc.link_id::text = j.payload #>> '{payload,recipient,link_id}')));
