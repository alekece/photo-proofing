-- A gallery holds two independent sets of photos: the proofs clients choose from and the finished
-- photos the photographer prepares privately. Both share the upload, processing and cleanup
-- pipeline, so they share the table; `kind` tells them apart and every proof query filters on it.
CREATE TYPE photo_kind AS ENUM ('proof', 'final');

-- `published_at` is set when a final enters the delivered set. `removed_at` marks a published
-- final the photographer removed from the draft: its row and files stay until the next publication.
ALTER TABLE photos
    ADD COLUMN kind photo_kind NOT NULL DEFAULT 'proof',
    ADD COLUMN published_at timestamptz,
    ADD COLUMN removed_at timestamptz,
    ADD CONSTRAINT photos_draft_columns_only_on_finals
        CHECK (kind = 'final' OR (published_at IS NULL AND removed_at IS NULL)),
    ADD CONSTRAINT photos_removed_only_when_published
        CHECK (removed_at IS NULL OR published_at IS NOT NULL);

-- Announces committed photo changes on channel `photo_events` for live dashboards. The payload
-- carries ids, the kind and the new status only; `deleted` marks a removal, including a final
-- leaving the draft.
CREATE OR REPLACE FUNCTION notify_photo_event() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    photo photos;
    change text;
BEGIN
    IF TG_OP = 'DELETE' THEN
        photo := OLD;
        change := 'deleted';
    ELSIF NEW.removed_at IS NOT NULL THEN
        photo := NEW;
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

CREATE TRIGGER photos_notify_removed
    AFTER UPDATE OF removed_at ON photos
    FOR EACH ROW WHEN (OLD.removed_at IS DISTINCT FROM NEW.removed_at)
    EXECUTE FUNCTION notify_photo_event();
