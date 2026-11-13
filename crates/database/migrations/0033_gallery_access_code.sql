-- Optional AEAD-encrypted 6-digit access code (12-byte nonce + 6-byte code + 16-byte tag).
ALTER TABLE galleries
    ADD COLUMN access_code_ciphertext bytea
        CONSTRAINT galleries_access_code_ciphertext_len
        CHECK (access_code_ciphertext IS NULL OR octet_length(access_code_ciphertext) = 34);
