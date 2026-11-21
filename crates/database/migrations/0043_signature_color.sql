ALTER TABLE workspace_settings
    ADD COLUMN signature_color text
        CHECK (signature_color ~ '^#[0-9A-F]{6}$');
