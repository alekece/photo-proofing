ALTER TABLE photos
    ADD COLUMN requested_watermark_revision bigint NOT NULL DEFAULT 0,
    ADD COLUMN watermark_only_pending boolean NOT NULL DEFAULT false;

-- Preserve the prior readiness comparison without assuming why any legacy job was queued.
UPDATE photos p
SET requested_watermark_revision = s.watermark_revision
FROM workspace_settings s
WHERE p.kind = 'proof' AND p.workspace_id = s.workspace_id
  AND s.watermark_revision <> 0;
