ALTER TABLE workspace_settings
    ADD COLUMN signature_text_color text
        CHECK (signature_text_color ~ '^#[0-9A-F]{6}$');
