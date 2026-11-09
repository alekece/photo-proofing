-- A favorite now remembers when the client picked it: `favorited_at` replaces the `favorite` flag
-- (NULL = not a favorite), like `downloaded_at`. The triggers move to the new column first, so
-- the backfill announces nothing.
DROP TRIGGER photos_notify_favorite ON photos;
DROP TRIGGER photos_notify_favorite_delete ON photos;
DROP TRIGGER photos_record_selection ON photos;
DROP TRIGGER photos_record_selection_delete ON photos;

ALTER TABLE photos ADD COLUMN favorited_at timestamptz;

-- The picking time was not recorded; the upload date is the best available stand-in.
UPDATE photos SET favorited_at = created_at WHERE favorite;

ALTER TABLE photos DROP COLUMN favorite;

-- Only a flip between favorite and not favorite is an event: picking an already picked photo
-- again keeps its date and announces nothing.
CREATE TRIGGER photos_notify_favorite
    AFTER UPDATE OF favorited_at ON photos
    FOR EACH ROW WHEN ((OLD.favorited_at IS NULL) <> (NEW.favorited_at IS NULL))
    EXECUTE FUNCTION notify_favorite_event();

CREATE TRIGGER photos_notify_favorite_delete
    AFTER DELETE ON photos
    FOR EACH ROW WHEN (OLD.favorited_at IS NOT NULL)
    EXECUTE FUNCTION notify_favorite_event();

CREATE TRIGGER photos_record_selection
    AFTER UPDATE OF favorited_at ON photos
    FOR EACH ROW WHEN ((OLD.favorited_at IS NULL) <> (NEW.favorited_at IS NULL))
    EXECUTE FUNCTION notify_selection_activity();

CREATE TRIGGER photos_record_selection_delete
    AFTER DELETE ON photos
    FOR EACH ROW WHEN (OLD.favorited_at IS NOT NULL)
    EXECUTE FUNCTION notify_selection_activity();
