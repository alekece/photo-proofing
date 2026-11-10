-- Threads retain deleted text for the photographer; deleting the photo erases the thread.
CREATE TABLE photo_comments (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    workspace_id uuid NOT NULL REFERENCES workspaces(id),
    gallery_id uuid NOT NULL REFERENCES galleries(id) ON DELETE CASCADE,
    photo_id uuid NOT NULL REFERENCES photos(id) ON DELETE CASCADE,
    author text NOT NULL CONSTRAINT photo_comments_author_known CHECK (author IN ('client', 'photographer')),
    body text NOT NULL CONSTRAINT photo_comments_body_length CHECK (char_length(body) BETWEEN 1 AND 1000),
    created_at timestamptz NOT NULL DEFAULT now(),
    edited_at timestamptz,
    deleted_at timestamptz
);

CREATE INDEX photo_comments_thread ON photo_comments(photo_id, created_at, id);
CREATE INDEX photo_comments_gallery ON photo_comments(workspace_id, gallery_id);

ALTER TABLE notifications DROP CONSTRAINT notifications_kind_known;
ALTER TABLE notifications ADD CONSTRAINT notifications_kind_known
    CHECK (kind IN ('selection', 'download', 'open', 'comment'));

CREATE FUNCTION notify_comment_activity() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM record_notification(NEW.workspace_id, NEW.gallery_id, 'comment', 1);

    RETURN NULL;
END;
$$;

CREATE TRIGGER photo_comments_record
    AFTER INSERT ON photo_comments
    FOR EACH ROW WHEN (NEW.author = 'client')
    EXECUTE FUNCTION notify_comment_activity();
