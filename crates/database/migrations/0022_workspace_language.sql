CREATE TYPE ui_language AS ENUM ('en', 'fr');

ALTER TABLE workspace_settings ADD COLUMN language ui_language NOT NULL DEFAULT 'en';
