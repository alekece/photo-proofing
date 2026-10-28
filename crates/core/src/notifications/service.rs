use crate::notifications::{
    Clock, GalleryId, Notification, NotificationStore, Photographer, SelectionActivity,
};

#[derive(Debug)]
pub struct NotificationService<S, C> {
    store: S,
    clock: C,
}

impl<S: NotificationStore, C: Clock> NotificationService<S, C> {
    pub fn new(store: S, clock: C) -> Self {
        Self { store, clock }
    }

    /// Records a favourite change and folds it into the gallery's unread
    /// notification, opening one if needed. Returns `false` for a duplicate
    /// (the photo is already in that state), which is ignored.
    pub fn record_activity(&mut self, activity: SelectionActivity) -> bool {
        let latest = self
            .store
            .latest_change(activity.gallery_id, activity.photo_id);
        if latest == Some(activity.change) {
            return false;
        }

        let mut notification = match self.store.find_unread(activity.gallery_id) {
            Some(notification) => notification,
            None => {
                let id = self.store.next_notification_id();

                Notification::open(id, activity.gallery_id, activity.occurred_at)
            }
        };
        notification.apply(&activity);

        self.store.append_activity(activity);
        self.store.save_notification(notification);

        true
    }

    /// Unread notifications with a non-zero net count, for the photographer.
    pub fn unread_notifications(&self, _photographer: &Photographer) -> Vec<Notification> {
        self.store
            .list_unread()
            .into_iter()
            .filter(|notification| notification.count() > 0)
            .collect()
    }

    pub fn history(
        &self,
        _photographer: &Photographer,
        gallery_id: GalleryId,
    ) -> Vec<SelectionActivity> {
        self.store.history(gallery_id)
    }

    /// Marks the gallery's unread notification read, as when the photographer
    /// opens the gallery. Returns the closed record, if there was one.
    pub fn mark_read(
        &mut self,
        _photographer: &Photographer,
        gallery_id: GalleryId,
    ) -> Option<Notification> {
        let mut notification = self.store.find_unread(gallery_id)?;
        notification.read_at = Some(self.clock.now());
        self.store.save_notification(notification.clone());

        Some(notification)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        rc::Rc,
        time::{Duration, SystemTime},
    };

    use uuid::Uuid;

    use super::*;
    use crate::notifications::{ClientSession, FavouriteChange, InMemoryStore, PhotoId, Principal};

    #[derive(Debug, Clone)]
    struct FakeClock(Rc<Cell<SystemTime>>);

    impl Clock for FakeClock {
        fn now(&self) -> SystemTime {
            self.0.get()
        }
    }

    const GALLERY: u128 = 1;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn activity(
        gallery: u128,
        photo: u128,
        change: FavouriteChange,
        secs: u64,
    ) -> SelectionActivity {
        SelectionActivity {
            gallery_id: GalleryId::from(Uuid::from_u128(gallery)),
            photo_id: PhotoId::from(Uuid::from_u128(photo)),
            change,
            occurred_at: at(secs),
        }
    }

    fn service() -> (
        NotificationService<InMemoryStore, FakeClock>,
        Rc<Cell<SystemTime>>,
    ) {
        let now = Rc::new(Cell::new(at(1000)));
        let service = NotificationService::new(InMemoryStore::default(), FakeClock(now.clone()));

        (service, now)
    }

    fn gallery() -> GalleryId {
        GalleryId::from(Uuid::from_u128(GALLERY))
    }

    #[test]
    fn repeated_activity_updates_one_notification() {
        let (mut service, _) = service();
        let me = Photographer::authenticated();

        for (photo, secs) in [(1, 10), (2, 20), (3, 30)] {
            service.record_activity(activity(GALLERY, photo, FavouriteChange::Added, secs));
        }

        let unread = service.unread_notifications(&me);
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].count(), 3);
        assert_eq!(unread[0].latest_activity_at, at(30));
    }

    #[test]
    fn galleries_are_grouped_separately() {
        let (mut service, _) = service();
        let me = Photographer::authenticated();

        service.record_activity(activity(1, 1, FavouriteChange::Added, 10));
        service.record_activity(activity(2, 1, FavouriteChange::Added, 20));

        assert_eq!(service.unread_notifications(&me).len(), 2);
    }

    #[test]
    fn duplicate_events_are_ignored() {
        let (mut service, _) = service();
        let me = Photographer::authenticated();

        assert!(service.record_activity(activity(GALLERY, 1, FavouriteChange::Added, 10)));
        assert!(!service.record_activity(activity(GALLERY, 1, FavouriteChange::Added, 20)));

        let unread = service.unread_notifications(&me);
        assert_eq!(unread[0].count(), 1);
        assert_eq!(unread[0].latest_activity_at, at(10));
        assert_eq!(service.history(&me, gallery()).len(), 1);
    }

    #[test]
    fn add_then_remove_nets_out_but_stays_in_history() {
        let (mut service, _) = service();
        let me = Photographer::authenticated();

        service.record_activity(activity(GALLERY, 1, FavouriteChange::Added, 10));
        service.record_activity(activity(GALLERY, 1, FavouriteChange::Removed, 20));
        assert!(service.unread_notifications(&me).is_empty());
        assert_eq!(service.history(&me, gallery()).len(), 2);

        service.record_activity(activity(GALLERY, 2, FavouriteChange::Added, 30));
        let unread = service.unread_notifications(&me);
        assert_eq!(unread[0].count(), 1);
        assert_eq!(unread[0].latest_activity_at, at(30));
    }

    #[test]
    fn mark_read_hides_notification_and_stamps_clock_time() {
        let (mut service, now) = service();
        let me = Photographer::authenticated();
        service.record_activity(activity(GALLERY, 1, FavouriteChange::Added, 10));
        now.set(at(2000));

        let read = service.mark_read(&me, gallery()).expect("an unread record");

        assert_eq!(read.read_at, Some(at(2000)));
        assert!(service.unread_notifications(&me).is_empty());
        assert!(service.mark_read(&me, gallery()).is_none());
    }

    #[test]
    fn activity_after_read_opens_a_new_record_counting_only_new_changes() {
        let (mut service, _) = service();
        let me = Photographer::authenticated();
        service.record_activity(activity(GALLERY, 1, FavouriteChange::Added, 10));
        service.record_activity(activity(GALLERY, 2, FavouriteChange::Added, 20));
        let first = service.mark_read(&me, gallery()).expect("an unread record");

        service.record_activity(activity(GALLERY, 3, FavouriteChange::Added, 30));
        service.record_activity(activity(GALLERY, 3, FavouriteChange::Added, 31));

        let unread = service.unread_notifications(&me);
        assert_eq!(unread.len(), 1);
        assert_ne!(unread[0].id, first.id);
        assert_eq!(unread[0].count(), 1);
        assert_eq!(unread[0].latest_activity_at, at(30));
    }

    #[test]
    fn removal_after_read_counts_as_a_change() {
        let (mut service, _) = service();
        let me = Photographer::authenticated();
        service.record_activity(activity(GALLERY, 1, FavouriteChange::Added, 10));
        service.mark_read(&me, gallery());

        service.record_activity(activity(GALLERY, 1, FavouriteChange::Removed, 20));

        assert_eq!(service.unread_notifications(&me)[0].count(), 1);
    }

    #[test]
    fn client_principal_cannot_obtain_read_access() {
        let client = Principal::Client(ClientSession::validated());
        let photographer = Principal::Photographer(Photographer::authenticated());

        assert!(client.as_photographer().is_none());
        assert!(photographer.as_photographer().is_some());
    }
}
