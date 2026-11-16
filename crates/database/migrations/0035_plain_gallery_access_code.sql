-- Replace AEAD ciphertext column with plain validated 6-digit access codes.
-- Existing locked galleries receive a fresh CSPRNG 6-digit code so they never silently unlock.
ALTER TABLE galleries
    ADD COLUMN access_code text
        CONSTRAINT galleries_access_code_six_digits
        CHECK (access_code IS NULL OR access_code ~ '^[0-9]{6}$');

UPDATE galleries
SET access_code = lpad(
    ((('x0000' || left(replace(gen_random_uuid()::text, '-', ''), 12))::bit(64)::bigint) % 1000000)::text,
    6,
    '0'
)
WHERE access_code_ciphertext IS NOT NULL;

ALTER TABLE galleries
    DROP COLUMN access_code_ciphertext;
