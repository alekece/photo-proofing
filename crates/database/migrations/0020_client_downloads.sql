-- Clients download their finals. A download is recorded when it is first requested, on the photo
-- and, for the delivered-set ZIP, on the export and on every final it holds.
ALTER TABLE photos ADD COLUMN downloaded_at timestamptz;

-- `selection` exports are the photographer's short-lived ZIPs of the client's favorites;
-- the `delivery` export is the client's ZIP of the delivered finals.
ALTER TABLE exports
    ADD COLUMN kind text NOT NULL DEFAULT 'selection'
        CONSTRAINT exports_kind_known CHECK (kind IN ('selection', 'delivery')),
    ADD COLUMN downloaded_at timestamptz;

-- Each kind has its own export being prepared, so one never blocks the other.
DROP INDEX exports_one_preparing;
CREATE UNIQUE INDEX exports_one_preparing
    ON exports(gallery_id, kind) WHERE status ->> 'kind' = 'preparing';

-- A first download is announced like any other change: ids only, on the photographer's stream.
CREATE TRIGGER photos_notify_downloaded
    AFTER UPDATE OF downloaded_at ON photos
    FOR EACH ROW WHEN (OLD.downloaded_at IS NULL AND NEW.downloaded_at IS NOT NULL)
    EXECUTE FUNCTION notify_photo_event();

CREATE TRIGGER exports_notify_downloaded
    AFTER UPDATE OF downloaded_at ON exports
    FOR EACH ROW WHEN (OLD.downloaded_at IS NULL AND NEW.downloaded_at IS NOT NULL)
    EXECUTE FUNCTION notify_export_event();
