-- The bell is now driven by `activity_events` (see 0037) instead of being written directly by
-- each table's own trigger. `photos_record_selection_delete` stays: a deleted favorite is still
-- the photographer's action and still notifies, and nothing records a delete as client activity.
DROP TRIGGER photos_record_selection ON photos;
DROP TRIGGER photos_record_download ON photos;
DROP FUNCTION notify_photo_download_activity();
DROP TRIGGER exports_record_download ON exports;
DROP FUNCTION notify_export_download_activity();
DROP TRIGGER galleries_record_opened ON galleries;
DROP FUNCTION notify_gallery_opened();
DROP TRIGGER photo_comments_record ON photo_comments;
DROP FUNCTION notify_comment_activity();
