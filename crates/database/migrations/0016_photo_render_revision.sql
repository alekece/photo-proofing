ALTER TABLE photos
    ADD COLUMN revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0);
