-- Sizes go from 5 steps to 10. Doubling keeps every saved watermark exactly as large as before.
ALTER TABLE workspace_settings DROP CONSTRAINT workspace_settings_size_range;
UPDATE workspace_settings SET size = size * 2;
ALTER TABLE workspace_settings
    ALTER COLUMN size SET DEFAULT 4,
    ADD CONSTRAINT workspace_settings_size_range CHECK (size BETWEEN 0 AND 9),
    ADD COLUMN signature_size smallint NOT NULL DEFAULT 4
        CONSTRAINT workspace_settings_signature_size_range CHECK (signature_size BETWEEN 0 AND 9);
