CREATE TYPE export_status AS ENUM ('preparing', 'ready', 'failed');

-- A photographer's request for a ZIP of the client's selection. The photos it holds are the
-- selection when it was requested; later favorite changes never touch them.
CREATE TABLE selection_exports (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
    status export_status NOT NULL DEFAULT 'preparing',
    failure_reason text,
    photo_count integer NOT NULL CONSTRAINT selection_exports_count_positive CHECK (photo_count > 0),
    size bigint NOT NULL CONSTRAINT selection_exports_size_positive CHECK (size > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    CONSTRAINT selection_exports_failure_reason_matches_status
        CHECK ((status = 'failed') = (failure_reason IS NOT NULL)),
    CONSTRAINT selection_exports_finished_unless_preparing
        CHECK ((status = 'preparing') = (finished_at IS NULL))
);

-- At most one export per gallery is being prepared.
CREATE UNIQUE INDEX selection_exports_one_preparing
    ON selection_exports(gallery_id) WHERE status = 'preparing';
CREATE INDEX selection_exports_gallery_created ON selection_exports(gallery_id, created_at DESC);

-- No foreign key to `photos`: deleting a photo must not rewrite the snapshot, the build notices it.
CREATE TABLE selection_export_photos (
    export_id uuid NOT NULL REFERENCES selection_exports(id) ON DELETE CASCADE,
    photo_id uuid NOT NULL,
    original_name text NOT NULL,
    position integer NOT NULL,
    PRIMARY KEY (export_id, photo_id)
);

-- Announces a changed export status on channel `export_events`, so the photographer's open
-- Favorites tab refetches it. Ids only.
CREATE FUNCTION notify_export_event() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('export_events', json_build_object(
        'workspace_id', NEW.workspace_id,
        'gallery_id', NEW.gallery_id
    )::text);

    RETURN NULL;
END;
$$;

CREATE TRIGGER selection_exports_notify
    AFTER INSERT OR UPDATE OF status ON selection_exports
    FOR EACH ROW EXECUTE FUNCTION notify_export_event();
