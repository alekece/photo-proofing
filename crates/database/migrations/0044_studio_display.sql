CREATE TYPE studio_mode AS ENUM ('full_image', 'print');
CREATE TYPE studio_palette AS ENUM ('warm', 'neutral');

ALTER TABLE workspace_settings
    ADD COLUMN studio_mode studio_mode NOT NULL DEFAULT 'full_image',
    ADD COLUMN studio_palette studio_palette NOT NULL DEFAULT 'warm';
