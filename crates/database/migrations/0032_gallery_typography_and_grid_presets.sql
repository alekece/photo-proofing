CREATE TYPE heading_font AS ENUM ('editorial', 'fine_art', 'timeless', 'modern');
CREATE TYPE grid_density AS ENUM ('compact', 'large');
CREATE TYPE grid_spacing AS ENUM ('compact', 'generous');

ALTER TABLE galleries
    ADD COLUMN heading_font heading_font NOT NULL DEFAULT 'editorial',
    ADD COLUMN grid_density grid_density NOT NULL DEFAULT 'compact',
    ADD COLUMN grid_spacing grid_spacing NOT NULL DEFAULT 'compact';
