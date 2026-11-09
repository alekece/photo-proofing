CREATE TYPE proof_quality AS ENUM ('low', 'medium', 'high');

ALTER TABLE workspace_settings ADD COLUMN proof_quality proof_quality NOT NULL DEFAULT 'medium';
-- The quality the clean thumbnail and preview were rendered at; unknown for older photos, which
-- are therefore rendered again the next time their proofs are regenerated.
ALTER TABLE photos ADD COLUMN clean_quality proof_quality;
