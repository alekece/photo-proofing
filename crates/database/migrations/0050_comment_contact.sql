ALTER TABLE photo_comments
    ADD COLUMN contact_id uuid REFERENCES contacts(id) ON DELETE SET NULL;

ALTER TABLE photo_comments
    ADD COLUMN contact_origin text NOT NULL DEFAULT 'general';

UPDATE photo_comments
SET contact_origin = CASE author
    WHEN 'photographer' THEN 'photographer'
    ELSE 'general'
END;

ALTER TABLE photo_comments
    ADD CONSTRAINT photo_comments_contact_is_client
    CHECK (
        (author = 'photographer'
            AND contact_origin = 'photographer'
            AND contact_id IS NULL)
        OR
        (author = 'client'
            AND contact_origin IN ('general', 'contact')
            AND (contact_origin = 'contact' OR contact_id IS NULL))
    );

ALTER TABLE photo_comments
    ADD CONSTRAINT photo_comments_contact_origin_known
    CHECK (contact_origin IN ('general', 'contact', 'photographer'));

CREATE INDEX photo_comments_contact ON photo_comments(contact_id) WHERE contact_id IS NOT NULL;
