//! Photographer-only unread notifications aggregated from favourite activity.
//!
//! Every favourite change is kept as history. Changes are grouped into one
//! unread [`Notification`] per gallery; once the photographer marks it read,
//! the next change opens a new record.
//!
//! Duplicate events (a change equal to the photo's latest recorded state) are
//! ignored, so each stored event flips its photo. The count is therefore the
//! number of photos whose favourite state differs from when the notification
//! was opened: add-then-remove nets out to zero.

use std::{collections::HashSet, time::SystemTime};

use derive_more::{Display, From};
use serde::{Deserialize, Serialize};
use strum::{Display as StrumDisplay, EnumString};
use uuid::Uuid;

use crate::types::generate::Generate;

mod service;
mod store;

pub use service::NotificationService;
pub use store::{InMemoryStore, NotificationStore};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From)]
pub struct GalleryId(Uuid);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From, Serialize, Deserialize,
)]
#[serde(transparent)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
pub struct PhotoId(Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Display, From)]
pub struct NotificationId(u64);

impl Generate for PhotoId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Generate for GalleryId {
    fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, StrumDisplay, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum FavouriteChange {
    Added,
    Removed,
}

/// A favourite change. Deliberately carries no client identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionActivity {
    pub gallery_id: GalleryId,
    pub photo_id: PhotoId,
    pub change: FavouriteChange,
    pub occurred_at: SystemTime,
}

/// Source of the current time, injectable for tests.
pub trait Clock {
    fn now(&self) -> SystemTime;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Proof of an authenticated photographer session.
///
/// Notification queries require it. The constructor is crate-private so
/// share-link code cannot forge it; the authentication layer will get a
/// controlled way to issue it when sessions are wired in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Photographer(());

impl Photographer {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "issued by the auth layer once sessions are wired in"
        )
    )]
    pub(crate) fn authenticated() -> Self {
        Self(())
    }
}

/// A share-link client session. It can never read notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSession(());

impl ClientSession {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "issued by the auth layer once sessions are wired in"
        )
    )]
    pub(crate) fn validated() -> Self {
        Self(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Principal {
    Photographer(Photographer),
    Client(ClientSession),
}

impl Principal {
    pub fn as_photographer(&self) -> Option<&Photographer> {
        match self {
            Self::Photographer(photographer) => Some(photographer),
            Self::Client(_) => None,
        }
    }
}

/// One gallery-level notification record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: NotificationId,
    pub gallery_id: GalleryId,
    pub latest_activity_at: SystemTime,
    pub read_at: Option<SystemTime>,
    // Photos with an odd number of changes since the record opened.
    flipped: HashSet<PhotoId>,
}

impl Notification {
    fn open(id: NotificationId, gallery_id: GalleryId, at: SystemTime) -> Self {
        Self {
            id,
            gallery_id,
            latest_activity_at: at,
            read_at: None,
            flipped: HashSet::new(),
        }
    }

    fn apply(&mut self, activity: &SelectionActivity) {
        if !self.flipped.remove(&activity.photo_id) {
            self.flipped.insert(activity.photo_id);
        }
        self.latest_activity_at = self.latest_activity_at.max(activity.occurred_at);
    }

    /// Photos whose favourite state changed since this record opened.
    pub fn count(&self) -> usize {
        self.flipped.len()
    }

    pub fn is_unread(&self) -> bool {
        self.read_at.is_none()
    }
}
