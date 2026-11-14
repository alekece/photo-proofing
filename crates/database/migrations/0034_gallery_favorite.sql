-- Photographer-only pin that sorts favorited galleries first on the dashboard.
ALTER TABLE galleries ADD COLUMN favorite boolean NOT NULL DEFAULT false;
