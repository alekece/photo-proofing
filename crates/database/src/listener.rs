//! PostgreSQL `LISTEN` for the photo, favorite and export event channels.
//!
//! PostgreSQL delivers a notification only when its transaction commits, and loses it while
//! the listening connection is down. A reconnect, like an undecodable payload, is therefore
//! reported as [`Notification::Missed`] so callers resynchronize instead of trusting a partial
//! history.

use sqlx::postgres::PgListener;

use crate::{
    PgPool, error::DatabaseError, export::ExportEvent, favorite_events::FavoriteEvent,
    photo_events::PhotoEvent,
};

/// The next thing a [`Listener`] has to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notification {
    Photo(PhotoEvent),
    Favorites(FavoriteEvent),
    Export(ExportEvent),
    /// Events may have happened without a notification: whoever shows them must refetch.
    Missed(MissedReason),
}

impl Notification {
    fn decode(channel: &str, payload: &str) -> Self {
        let decoded = match channel {
            PhotoEvent::CHANNEL => serde_json::from_str(payload).map(Self::Photo),
            FavoriteEvent::CHANNEL => serde_json::from_str(payload).map(Self::Favorites),
            ExportEvent::CHANNEL => serde_json::from_str(payload).map(Self::Export),
            _ => return Self::Missed(MissedReason::Undecodable),
        };

        decoded.unwrap_or(Self::Missed(MissedReason::Undecodable))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissedReason {
    /// The listening connection was lost and has been reopened.
    Reconnected,
    /// A notification was not a valid payload for its channel.
    Undecodable,
}

/// A dedicated connection listening on every event channel; it holds one pool connection for
/// its whole life.
pub struct Listener {
    listener: PgListener,
}

impl Listener {
    /// Opens a dedicated connection from `pool` and listens on every event channel. Only
    /// notifications committed after this returns are reported.
    pub(crate) async fn open(pool: &PgPool) -> Result<Self, DatabaseError> {
        let mut listener = PgListener::connect_with(pool).await?;

        listener
            .listen_all([
                PhotoEvent::CHANNEL,
                FavoriteEvent::CHANNEL,
                ExportEvent::CHANNEL,
            ])
            .await?;

        Ok(Self { listener })
    }

    /// Waits for the next notification.
    ///
    /// A lost connection is reopened before this returns [`MissedReason::Reconnected`]; an error
    /// means it could not be reopened, and the listener should be replaced.
    pub async fn next(&mut self) -> Result<Notification, DatabaseError> {
        let Some(notification) = self.listener.try_recv().await? else {
            return Ok(Notification::Missed(MissedReason::Reconnected));
        };

        Ok(Notification::decode(
            notification.channel(),
            notification.payload(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE: &str = "00000000-0000-0000-0000-000000000001";
    const GALLERY: &str = "00000000-0000-0000-0000-000000000002";

    #[test]
    fn a_payload_is_decoded_by_its_channel() {
        let payload = format!(r#"{{"workspace_id":"{WORKSPACE}","gallery_id":"{GALLERY}"}}"#);

        assert!(matches!(
            Notification::decode(FavoriteEvent::CHANNEL, &payload),
            Notification::Favorites(_)
        ));
        assert_eq!(
            Notification::decode(PhotoEvent::CHANNEL, &payload),
            Notification::Missed(MissedReason::Undecodable),
            "a favorite payload is not a photo payload"
        );
    }

    #[test]
    fn a_bad_payload_or_unknown_channel_is_missed() {
        for (channel, payload) in [
            (PhotoEvent::CHANNEL, "not json"),
            (ExportEvent::CHANNEL, "{}"),
            ("other_events", "{}"),
        ] {
            assert_eq!(
                Notification::decode(channel, payload),
                Notification::Missed(MissedReason::Undecodable),
                "{channel}"
            );
        }
    }
}
