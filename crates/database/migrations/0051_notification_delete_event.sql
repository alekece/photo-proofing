-- A deleted notification is announced like a changed one, so open browsers refetch the list.
CREATE FUNCTION notify_notification_deleted() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('notification_events', json_build_object(
        'workspace_id', OLD.workspace_id,
        'gallery_id', OLD.gallery_id
    )::text);

    RETURN NULL;
END;
$$;

CREATE TRIGGER notifications_notify_deleted
    AFTER DELETE ON notifications
    FOR EACH ROW EXECUTE FUNCTION notify_notification_deleted();
