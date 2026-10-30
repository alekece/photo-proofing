-- A gallery is shared once it has a token: 128 random bits as 22 unpadded URL-safe base64 characters.
ALTER TABLE galleries
    ADD COLUMN share_token text
        CONSTRAINT galleries_share_token_shape CHECK (share_token ~ '^[A-Za-z0-9_-]{22}$')
        CONSTRAINT galleries_share_token_unique UNIQUE;
