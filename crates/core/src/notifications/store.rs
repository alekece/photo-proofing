use std::collections::HashMap;

use crate::notifications::{
    FavouriteChange, GalleryId, Notification, NotificationId, PhotoId, SelectionActivity,
};

/// Persistence boundary; a database-backed implementation arrives later.
pub trait NotificationStore {
    fn append_activity(&mut self, activity: SelectionActivity);

    fn latest_change(&self, gallery_id: GalleryId, photo_id: PhotoId) -> Option<FavouriteChange>;

    fn history(&self, gallery_id: GalleryId) -> Vec<SelectionActivity>;

    fn next_notification_id(&mut self) -> NotificationId;

    /// The gallery's notification that has not been read yet, if any.
    fn find_unread(&self, gallery_id: GalleryId) -> Option<Notification>;

    fn list_unread(&self) -> Vec<Notification>;

    fn save_notification(&mut self, notification: Notification);
}

#[derive(Debug, Default)]
pub struct InMemoryStore {
    activities: Vec<SelectionActivity>,
    notifications: Vec<Notification>,
    latest_changes: HashMap<(GalleryId, PhotoId), FavouriteChange>,
    next_id: u64,
}

impl NotificationStore for InMemoryStore {
    fn append_activity(&mut self, activity: SelectionActivity) {
        self.latest_changes
            .insert((activity.gallery_id, activity.photo_id), activity.change);
        self.activities.push(activity);
    }

    fn latest_change(&self, gallery_id: GalleryId, photo_id: PhotoId) -> Option<FavouriteChange> {
        self.latest_changes.get(&(gallery_id, photo_id)).copied()
    }

    fn history(&self, gallery_id: GalleryId) -> Vec<SelectionActivity> {
        self.activities
            .iter()
            .filter(|activity| activity.gallery_id == gallery_id)
            .copied()
            .collect()
    }

    fn next_notification_id(&mut self) -> NotificationId {
        self.next_id += 1;

        NotificationId::from(self.next_id)
    }

    fn find_unread(&self, gallery_id: GalleryId) -> Option<Notification> {
        self.notifications
            .iter()
            .find(|notification| notification.gallery_id == gallery_id && notification.is_unread())
            .cloned()
    }

    fn list_unread(&self) -> Vec<Notification> {
        self.notifications
            .iter()
            .filter(|notification| notification.is_unread())
            .cloned()
            .collect()
    }

    fn save_notification(&mut self, notification: Notification) {
        match self
            .notifications
            .iter_mut()
            .find(|stored| stored.id == notification.id)
        {
            Some(stored) => *stored = notification,
            None => self.notifications.push(notification),
        }
    }
}
