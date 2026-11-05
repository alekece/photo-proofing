-- A gallery's progress and archive state are recorded as when they happened. `delivered_at` and
-- `archived_at` are written by later features; `shared_at` is set when the private link is created.
ALTER TABLE galleries
    ADD COLUMN shared_at timestamptz,
    ADD COLUMN delivered_at timestamptz,
    ADD COLUMN archived_at timestamptz;

-- Links created before this migration have no recorded time; `updated_at` is the closest estimate.
UPDATE galleries SET shared_at = updated_at WHERE share_token IS NOT NULL;
