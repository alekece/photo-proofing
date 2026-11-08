-- What clients did, for the photographer's bell: at most one unread row per gallery, which
-- accumulates activity until it is read. Activity after a read starts a new row.
CREATE TABLE notifications (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
    kind text NOT NULL CONSTRAINT notifications_kind_known CHECK (kind IN ('selection', 'download')),
    -- Newly downloaded finals; always 0 for a selection, whose text uses the live count.
    count integer NOT NULL DEFAULT 0 CONSTRAINT notifications_count_not_negative CHECK (count >= 0),
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    read_at timestamptz
);

CREATE UNIQUE INDEX notifications_one_unread ON notifications(gallery_id) WHERE read_at IS NULL;
CREATE INDEX notifications_workspace_updated ON notifications(workspace_id, updated_at DESC);

-- Adds activity to the gallery's unread row, or opens one. The row is locked by the upsert, so a
-- concurrent read either includes this activity or leaves it to a new row. A kind change restarts
-- the count. A gallery that is being deleted records nothing.
CREATE FUNCTION record_notification(w uuid, g uuid, k text, added integer) RETURNS void
LANGUAGE sql AS $$
    INSERT INTO notifications (workspace_id, gallery_id, kind, count)
    SELECT w, g, k, added FROM galleries WHERE id = g
    ON CONFLICT (gallery_id) WHERE read_at IS NULL DO UPDATE SET
        count = CASE WHEN notifications.kind = EXCLUDED.kind
                     THEN notifications.count + EXCLUDED.count ELSE EXCLUDED.count END,
        kind = EXCLUDED.kind,
        updated_at = now();
$$;

CREATE FUNCTION notify_selection_activity() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    photo photos;
BEGIN
    IF TG_OP = 'DELETE' THEN
        photo := OLD;
    ELSE
        photo := NEW;
    END IF;

    PERFORM record_notification(photo.workspace_id, photo.gallery_id, 'selection', 0);

    RETURN NULL;
END;
$$;

CREATE FUNCTION notify_photo_download_activity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM record_notification(NEW.workspace_id, NEW.gallery_id, 'download', 1);

    RETURN NULL;
END;
$$;

CREATE FUNCTION notify_export_download_activity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM record_notification(NEW.workspace_id, NEW.gallery_id, 'download', 0);

    RETURN NULL;
END;
$$;

CREATE TRIGGER photos_record_selection
    AFTER UPDATE OF favorite ON photos
    FOR EACH ROW WHEN (OLD.favorite IS DISTINCT FROM NEW.favorite)
    EXECUTE FUNCTION notify_selection_activity();

CREATE TRIGGER photos_record_selection_delete
    AFTER DELETE ON photos
    FOR EACH ROW WHEN (OLD.favorite)
    EXECUTE FUNCTION notify_selection_activity();

CREATE TRIGGER photos_record_download
    AFTER UPDATE OF downloaded_at ON photos
    FOR EACH ROW WHEN (NEW.kind = 'final' AND OLD.downloaded_at IS NULL AND NEW.downloaded_at IS NOT NULL)
    EXECUTE FUNCTION notify_photo_download_activity();

CREATE TRIGGER exports_record_download
    AFTER UPDATE OF downloaded_at ON exports
    FOR EACH ROW WHEN (NEW.kind = 'delivery' AND OLD.downloaded_at IS NULL AND NEW.downloaded_at IS NOT NULL)
    EXECUTE FUNCTION notify_export_download_activity();

-- Announces a changed notification on channel `notification_events`, ids only.
CREATE FUNCTION notify_notification_event() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('notification_events', json_build_object(
        'workspace_id', NEW.workspace_id,
        'gallery_id', NEW.gallery_id
    )::text);

    RETURN NULL;
END;
$$;

CREATE TRIGGER notifications_notify
    AFTER INSERT OR UPDATE ON notifications
    FOR EACH ROW EXECUTE FUNCTION notify_notification_event();
