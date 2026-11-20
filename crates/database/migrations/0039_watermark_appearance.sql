ALTER TABLE workspace_settings
    ADD COLUMN watermark_opacity smallint NOT NULL DEFAULT 50
        CHECK (watermark_opacity BETWEEN 0 AND 100),
    ADD COLUMN watermark_text_color text NOT NULL DEFAULT '#FFFFFF'
        CHECK (watermark_text_color ~ '^#[0-9A-F]{6}$');
