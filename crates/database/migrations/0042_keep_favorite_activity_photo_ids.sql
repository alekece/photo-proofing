-- Historical rows cannot recover any photo IDs that were discarded by the old recorder.
ALTER TABLE activity_events
    DROP CONSTRAINT activity_events_photo_ids_capped,
    ADD CONSTRAINT activity_events_photo_ids_capped
        CHECK (kind IN ('favorite', 'unfavorite') OR cardinality(photo_ids) <= 4);
