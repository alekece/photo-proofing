-- Delivery is the publication: once a gallery is delivered, its ready finals are the client's set,
-- so finals no longer carry a publication time or a soft removal. Removing a final deletes it.
DROP TRIGGER photos_notify_removed ON photos;

-- Soft-removed rows could only exist after a publication, which nothing ever recorded.
DELETE FROM photos WHERE removed_at IS NOT NULL;

ALTER TABLE photos
    DROP CONSTRAINT photos_removed_only_when_published,
    DROP CONSTRAINT photos_draft_columns_only_on_finals,
    DROP COLUMN published_at,
    DROP COLUMN removed_at;

CREATE OR REPLACE FUNCTION notify_photo_event() RETURNS trigger
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
        'kind', photo.kind,
        'status', change
    )::text);

    RETURN NULL;
END;
$$;
