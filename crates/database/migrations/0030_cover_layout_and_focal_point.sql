CREATE TYPE cover_layout AS ENUM ('full_bleed', 'split', 'framed', 'banner');

-- Cover template and percentage crop anchor (0..=100 from top-left; defaults keep portraits' faces framed).
ALTER TABLE galleries
    ADD COLUMN cover_layout cover_layout NOT NULL DEFAULT 'full_bleed',
    ADD COLUMN hero_focal_x smallint NOT NULL DEFAULT 50,
    ADD COLUMN hero_focal_y smallint NOT NULL DEFAULT 35,
    ADD CONSTRAINT galleries_hero_focal_point_range
        CHECK (hero_focal_x BETWEEN 0 AND 100 AND hero_focal_y BETWEEN 0 AND 100);
