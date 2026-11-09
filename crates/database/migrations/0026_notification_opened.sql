-- A client opening the shared gallery is a third kind of notification. Like the others it adds to
-- the gallery's unread row, one per recorded open: opens inside the throttle never reach this
-- column. Only a later open fires it, so moving the date back changes nothing.
ALTER TABLE notifications DROP CONSTRAINT notifications_kind_known;
ALTER TABLE notifications ADD CONSTRAINT notifications_kind_known
    CHECK (kind IN ('selection', 'download', 'open'));

CREATE FUNCTION notify_gallery_opened() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM record_notification(NEW.workspace_id, NEW.id, 'open', 1);

    RETURN NULL;
END;
$$;

CREATE TRIGGER galleries_record_opened
    AFTER UPDATE OF last_opened_at ON galleries
    FOR EACH ROW WHEN (NEW.last_opened_at IS NOT NULL
                       AND (OLD.last_opened_at IS NULL OR NEW.last_opened_at > OLD.last_opened_at))
    EXECUTE FUNCTION notify_gallery_opened();
