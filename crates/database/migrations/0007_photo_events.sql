-- Announces committed photo changes on channel `photo_events` for live dashboards.
-- The payload carries ids and the new status only; `deleted` marks a removal.
CREATE FUNCTION notify_photo_event() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    photo photos;
    change text;
BEGIN
    IF TG_OP = 'DELETE' THEN
        photo := OLD;
        change := 'deleted';
    ELSE
        photo := NEW;
        change := NEW.status::text;
    END IF;

    PERFORM pg_notify('photo_events', json_build_object(
        'workspace_id', photo.workspace_id,
        'gallery_id', photo.gallery_id,
        'photo_id', photo.id,
        'status', change
    )::text);

    RETURN NULL;
END;
$$;

CREATE TRIGGER photos_notify_insert_delete
    AFTER INSERT OR DELETE ON photos
    FOR EACH ROW EXECUTE FUNCTION notify_photo_event();

CREATE TRIGGER photos_notify_status
    AFTER UPDATE OF status ON photos
    FOR EACH ROW WHEN (OLD.status IS DISTINCT FROM NEW.status)
    EXECUTE FUNCTION notify_photo_event();
