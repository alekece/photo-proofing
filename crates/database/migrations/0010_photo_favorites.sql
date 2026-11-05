-- The client's selection is one flag per photo: a gallery has a single shared selection, so a
-- deleted photo takes its favorite with it and a favorite cannot point at another gallery.
ALTER TABLE photos ADD COLUMN favorite boolean NOT NULL DEFAULT false;

-- Announces a changed selection on channel `favorite_events`, so every browser using the link and
-- the photographer's dashboard refetch it. Ids only; deleting a favorite photo changes it too.
CREATE FUNCTION notify_favorite_event() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    photo photos;
BEGIN
    IF TG_OP = 'DELETE' THEN
        photo := OLD;
    ELSE
        photo := NEW;
    END IF;

    PERFORM pg_notify('favorite_events', json_build_object(
        'workspace_id', photo.workspace_id,
        'gallery_id', photo.gallery_id
    )::text);

    RETURN NULL;
END;
$$;

CREATE TRIGGER photos_notify_favorite
    AFTER UPDATE OF favorite ON photos
    FOR EACH ROW WHEN (OLD.favorite IS DISTINCT FROM NEW.favorite)
    EXECUTE FUNCTION notify_favorite_event();

CREATE TRIGGER photos_notify_favorite_delete
    AFTER DELETE ON photos
    FOR EACH ROW WHEN (OLD.favorite)
    EXECUTE FUNCTION notify_favorite_event();
