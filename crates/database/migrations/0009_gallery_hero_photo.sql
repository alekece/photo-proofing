ALTER TABLE galleries
    ADD COLUMN hero_photo_id uuid REFERENCES photos(id) ON DELETE SET NULL;
