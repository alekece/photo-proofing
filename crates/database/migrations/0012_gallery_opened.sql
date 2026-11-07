-- The first time a client opened the shared gallery. Set once and never changed; galleries
-- opened before this column existed stay NULL because those opens were not observed.
ALTER TABLE galleries ADD COLUMN opened_at timestamptz;
