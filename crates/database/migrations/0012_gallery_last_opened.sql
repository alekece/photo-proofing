-- When a client last opened the shared gallery, refreshed at most once per throttle delay.
-- Galleries opened before this column existed stay NULL because those opens were not observed.
ALTER TABLE galleries ADD COLUMN last_opened_at timestamptz;
