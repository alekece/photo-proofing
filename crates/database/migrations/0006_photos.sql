CREATE TYPE photo_status AS ENUM ('waiting', 'processing', 'ready', 'failed');

CREATE TABLE photos (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
    original_name text NOT NULL,
    size bigint NOT NULL CONSTRAINT photos_size_positive CHECK (size > 0),
    width integer NOT NULL CONSTRAINT photos_width_positive CHECK (width > 0),
    height integer NOT NULL CONSTRAINT photos_height_positive CHECK (height > 0),
    status photo_status NOT NULL DEFAULT 'waiting',
    failure_reason text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX photos_workspace_gallery_created
    ON photos(workspace_id, gallery_id, created_at, id);
