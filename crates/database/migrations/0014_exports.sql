-- A photographer's request for a ZIP of the client's selection. The photos it holds are the
-- selection when it was requested; later favorite changes never touch them.
CREATE TABLE exports (
    id uuid PRIMARY KEY,
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
    -- A tagged value: {"kind": "preparing"}, {"kind": "ready"} or {"kind": "failed", "reason": "..."}.
    status jsonb NOT NULL DEFAULT '{"kind": "preparing"}',
    photo_count integer NOT NULL CONSTRAINT exports_count_positive CHECK (photo_count > 0),
    size bigint NOT NULL CONSTRAINT exports_size_positive CHECK (size > 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz,
    CONSTRAINT exports_status_is_tagged CHECK (
        status IN ('{"kind": "preparing"}', '{"kind": "ready"}')
        OR (status ->> 'kind' = 'failed'
            AND coalesce(jsonb_typeof(status -> 'reason'), '') = 'string'
            AND status - 'kind' - 'reason' = '{}')
    ),
    CONSTRAINT exports_finished_unless_preparing
        CHECK ((status ->> 'kind' = 'preparing') = (finished_at IS NULL))
);

-- At most one export per gallery is being prepared.
CREATE UNIQUE INDEX exports_one_preparing
    ON exports(gallery_id) WHERE status ->> 'kind' = 'preparing';
CREATE INDEX exports_gallery_created ON exports(gallery_id, created_at DESC);

-- No foreign key to `photos`: deleting a photo must not rewrite the snapshot, the build notices it.
CREATE TABLE export_photos (
    export_id uuid NOT NULL REFERENCES exports(id) ON DELETE CASCADE,
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

CREATE TRIGGER exports_notify
    AFTER INSERT OR UPDATE OF status ON exports
    FOR EACH ROW EXECUTE FUNCTION notify_export_event();
