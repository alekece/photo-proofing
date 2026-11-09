-- A failed export now stores a code (`attempts_exhausted`, `photo_removed` or `disk_full`)
-- instead of an English sentence. Rows already in a code are left alone, so this can rerun.
UPDATE exports
SET status = jsonb_build_object(
    'kind', 'failed',
    'reason', CASE status ->> 'reason'
        WHEN 'A selected photo was removed. Try again.' THEN 'photo_removed'
        WHEN 'The server is out of disk space.' THEN 'disk_full'
        WHEN 'photo_removed' THEN 'photo_removed'
        WHEN 'disk_full' THEN 'disk_full'
        ELSE 'attempts_exhausted'
    END)
WHERE status ->> 'kind' = 'failed';
