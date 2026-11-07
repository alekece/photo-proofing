CREATE TYPE watermark_mark AS ENUM ('name', 'logo', 'both');
CREATE TYPE watermark_pattern AS ENUM ('repeat', 'once');
CREATE TYPE cover_signature AS ENUM ('name', 'logo', 'both', 'none');

CREATE TABLE workspace_settings (
    workspace_id uuid PRIMARY KEY REFERENCES workspaces(id) ON DELETE CASCADE,
    studio_name text NOT NULL DEFAULT ''
        CONSTRAINT workspace_settings_studio_name_length CHECK (char_length(studio_name) <= 100),
    mark watermark_mark NOT NULL DEFAULT 'name',
    pattern watermark_pattern NOT NULL DEFAULT 'repeat',
    position smallint NOT NULL DEFAULT 8
        CONSTRAINT workspace_settings_position_range CHECK (position BETWEEN 0 AND 8),
    size smallint NOT NULL DEFAULT 2
        CONSTRAINT workspace_settings_size_range CHECK (size BETWEEN 0 AND 4),
    signature cover_signature NOT NULL DEFAULT 'name',
    -- Normalised PNG; the application bounds it well below this sanity limit.
    logo bytea CONSTRAINT workspace_settings_logo_size CHECK (octet_length(logo) <= 4194304),
    -- Increases whenever the drawn watermark changes, so a render can tell it is out of date.
    watermark_revision bigint NOT NULL DEFAULT 0,
    updated_at timestamptz NOT NULL DEFAULT now()
);
