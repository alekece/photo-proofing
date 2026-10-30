CREATE TYPE gallery_layout AS ENUM ('grid', 'masonry');

CREATE TABLE galleries (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    title text NOT NULL CONSTRAINT galleries_title_length CHECK (char_length(title) BETWEEN 1 AND 255),
    layout gallery_layout NOT NULL DEFAULT 'grid',
    expires_at timestamptz,
    included_photos integer CONSTRAINT galleries_included_photos_positive CHECK (included_photos >= 1),
    watermark_enabled boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX galleries_workspace ON galleries(workspace_id);
