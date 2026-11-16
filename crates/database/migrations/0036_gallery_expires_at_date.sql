ALTER TABLE galleries
    ALTER COLUMN expires_at TYPE date
    USING (expires_at AT TIME ZONE 'UTC')::date;
