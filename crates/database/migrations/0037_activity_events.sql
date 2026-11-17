-- The workspace activity log: what each client did across galleries, attributed to the contact
-- behind their personal link (NULL for the general link's anonymous visitors). Rapid repeats of
-- the same gallery, contact and kind extend one line instead of opening a new one; a recorder in
-- `pp-database` owns that coalescing, so this migration only shapes the table.
CREATE TABLE activity_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
    -- A deleted contact turns its past lines anonymous instead of removing them.
    contact_id uuid REFERENCES contacts(id) ON DELETE SET NULL,
    kind text NOT NULL CONSTRAINT activity_events_kind_known
        CHECK (kind IN ('open', 'favorite', 'unfavorite', 'comment', 'download', 'email')),
    count integer NOT NULL CONSTRAINT activity_events_count_not_negative CHECK (count >= 0),
    -- At most the first four photos of the line, for thumbnails; a download batch may affect
    -- more than that without growing this array.
    photo_ids uuid[] NOT NULL DEFAULT '{}'
        CONSTRAINT activity_events_photo_ids_capped CHECK (coalesce(array_length(photo_ids, 1), 0) <= 4),
    detail text,
    first_at timestamptz NOT NULL DEFAULT now(),
    last_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX activity_events_workspace_last ON activity_events (workspace_id, last_at DESC, id DESC);
CREATE INDEX activity_events_gallery_last ON activity_events (gallery_id, last_at DESC, id DESC);

-- Derives the bell from the log, replacing nothing yet: the five notification triggers it will
-- eventually replace stay in place for now. A favorite or unfavorite line always notifies with
-- count 0, as the selection notification does today; a comment, open or download line notifies
-- with the change in `count`; an email line notifies nothing.
CREATE FUNCTION notify_activity_bell() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    notification_kind text;
    added integer := NEW.count - coalesce(OLD.count, 0);
BEGIN
    IF NEW.kind IN ('favorite', 'unfavorite') THEN
        notification_kind := 'selection';
        added := 0;
    ELSIF NEW.kind = 'comment' THEN
        notification_kind := 'comment';
    ELSIF NEW.kind = 'open' THEN
        notification_kind := 'open';
    ELSIF NEW.kind = 'download' THEN
        notification_kind := 'download';
    ELSE
        notification_kind := NULL;
    END IF;

    IF notification_kind IS NOT NULL THEN
        PERFORM record_notification(NEW.workspace_id, NEW.gallery_id, notification_kind, added);
    END IF;

    RETURN NULL;
END;
$$;

CREATE TRIGGER activity_events_record_notification
    AFTER INSERT OR UPDATE OF count ON activity_events
    FOR EACH ROW EXECUTE FUNCTION notify_activity_bell();

-- Reuses the existing channel and payload shape: a bell-worthy line also fires
-- `notify_notification_event()` through `record_notification`, with an identical payload, so
-- PostgreSQL coalesces the two into the single notification a listener receives.
CREATE TRIGGER activity_events_notify
    AFTER INSERT OR UPDATE ON activity_events
    FOR EACH ROW EXECUTE FUNCTION notify_notification_event();
