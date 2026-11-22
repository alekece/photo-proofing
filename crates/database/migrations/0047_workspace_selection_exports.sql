-- Older single-gallery snapshots already identify the source through their export.
ALTER TABLE export_photos ADD COLUMN source_gallery_id uuid;

ALTER TABLE exports
    DROP CONSTRAINT exports_kind_known,
    ADD CONSTRAINT exports_kind_known CHECK (kind IN ('selection', 'final_selection', 'delivery', 'workspace_selection'));

CREATE UNIQUE INDEX exports_one_workspace_selection_preparing ON exports(workspace_id)
    WHERE kind = 'workspace_selection' AND status ->> 'kind' = 'preparing';

-- Removing the storage gallery cancels its job; wake the tab waiting for completion.
CREATE FUNCTION notify_removed_workspace_export() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('export_events', json_build_object(
        'workspace_id', OLD.workspace_id, 'gallery_id', OLD.gallery_id
    )::text);
    RETURN NULL;
END;
$$;

CREATE TRIGGER exports_notify_removed_workspace_selection
    AFTER DELETE ON exports FOR EACH ROW
    WHEN (OLD.kind = 'workspace_selection' AND OLD.status ->> 'kind' = 'preparing')
    EXECUTE FUNCTION notify_removed_workspace_export();
