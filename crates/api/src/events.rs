//! Live photo, favorite and export events, from the database to every open event stream.
//!
//! One [`EventHub::relay`] task per process listens to the database and publishes on the hub;
//! each `GET /api/v1/events` stream subscribes and forwards its own workspace's events (see
//! [`routes::events`](crate::routes::events)). Whenever events may have been lost (a reconnect,
//! an undecodable notification or a lagging subscriber), streams receive [`Broadcast::Resync`]
//! so the browser refetches instead of trusting a partial history.

use std::time::Duration;

use pp_database::{
    Database,
    error::DatabaseError,
    export::ExportEvent,
    favorite_events::FavoriteEvent,
    listener::{Listener, MissedReason, Notification},
    photo_events::PhotoEvent,
};
use tokio::{sync::broadcast, time};
use tokio_util::sync::CancellationToken;

use crate::config::{EventsConfig, KeepAlive};

/// What the hub sends every subscriber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Broadcast {
    Photo(PhotoEvent),
    /// A gallery's selection changed.
    Favorites(FavoriteEvent),
    /// A selection ZIP started, finished or failed.
    Export(ExportEvent),
    /// Events may have been lost: subscribers must refetch.
    Resync,
}

impl From<Notification> for Broadcast {
    fn from(notification: Notification) -> Self {
        match notification {
            Notification::Photo(event) => Self::Photo(event),
            Notification::Favorites(event) => Self::Favorites(event),
            Notification::Export(event) => Self::Export(event),
            Notification::Missed(_) => Self::Resync,
        }
    }
}

/// Fans database events out to the open streams.
///
/// Streams are not counted or limited: they all share one bounded buffer, so a browser that
/// stops reading only lags and is told to resync.
pub(crate) struct EventHub {
    sender: broadcast::Sender<Broadcast>,
    cancellation_token: CancellationToken,
    keep_alive: KeepAlive,
    relay_backoff_start: Duration,
    relay_backoff_cap: Duration,
}

impl EventHub {
    pub fn new(config: &EventsConfig) -> Self {
        let (sender, _) = broadcast::channel(usize::from(config.capacity.get()));

        Self {
            sender,
            cancellation_token: CancellationToken::new(),
            keep_alive: config.keep_alive,
            relay_backoff_start: config.relay_backoff_start.get(),
            relay_backoff_cap: config
                .relay_backoff_cap
                .get()
                .max(config.relay_backoff_start.get()),
        }
    }

    pub fn keep_alive(&self) -> Duration {
        self.keep_alive.get()
    }

    pub fn subscribe(&self) -> Subscription {
        Subscription {
            receiver: self.sender.subscribe(),
            cancellation_token: self.cancellation_token.clone(),
        }
    }

    /// Ends every open stream. A stream otherwise lasts as long as its session, so the server
    /// could not stop gracefully while a dashboard is open.
    pub fn close(&self) {
        self.cancellation_token.cancel();
    }

    pub fn publish(&self, broadcast: Broadcast) {
        // Sending fails only when no stream is open, and then nobody needs the event.
        let _ = self.sender.send(broadcast);
    }

    /// Listens to the database for the life of the process and publishes every photo, favorite
    /// and export change.
    ///
    /// A failure is retried with a growing delay and never ends the task, so the owner aborts it
    /// on shutdown. Each new connection starts with a resync, since changes made while no
    /// listener was open were not announced.
    pub async fn relay(&self, database: Database) {
        let mut backoff = self.relay_backoff_start;

        loop {
            match database.listen().await {
                Ok(listener) => {
                    backoff = self.relay_backoff_start;

                    self.publish(Broadcast::Resync);

                    let error = self.forward(listener).await;

                    tracing::warn!(%error, "event listener failed");
                }
                Err(error) => {
                    tracing::warn!(%error, "event listener unavailable");
                }
            }

            time::sleep(backoff).await;
            backoff = (backoff * 2).min(self.relay_backoff_cap);
        }
    }

    /// Publishes `listener`'s notifications until it fails.
    async fn forward(&self, mut listener: Listener) -> DatabaseError {
        loop {
            match listener.next().await {
                Ok(Notification::Missed(MissedReason::Undecodable)) => {
                    tracing::warn!("undecodable event notification");
                    self.publish(Broadcast::Resync);
                }
                Ok(notification) => self.publish(notification.into()),
                Err(error) => return error,
            }
        }
    }
}

/// A stream's receiver of the hub's broadcasts.
pub(crate) struct Subscription {
    pub receiver: broadcast::Receiver<Broadcast>,
    /// Cancelled when the hub closes: the stream must end so the server can stop.
    pub cancellation_token: CancellationToken,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use pp_database::{
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        photo::{PhotoId, PhotoKind},
        photo_events::PhotoChange,
        tests::TestRepository,
    };
    use serde_json::json;
    use sqlx::PgPool;
    use tokio::task::JoinHandle;
    use uuid::Uuid;

    use super::*;
    use crate::tests::events_config;

    async fn recv(subscription: &mut Subscription) -> Broadcast {
        time::timeout(Duration::from_secs(5), subscription.receiver.recv())
            .await
            .expect("a broadcast arrives")
            .unwrap()
    }

    struct Relay {
        task: JoinHandle<()>,
    }

    impl Relay {
        /// Starts a relay and waits until its listener is open, signalled by the first resync.
        async fn start(pool: &PgPool) -> (Self, Subscription) {
            let hub = Arc::new(EventHub::new(&events_config()));
            let mut subscription = hub.subscribe();
            let database = Database::new(pool.clone());
            let task = tokio::spawn({
                let hub = Arc::clone(&hub);

                async move { hub.relay(database).await }
            });

            assert_eq!(recv(&mut subscription).await, Broadcast::Resync);

            (Self { task }, subscription)
        }
    }

    impl Drop for Relay {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn the_relay_publishes_committed_photo_changes(pool: PgPool) {
        let (_relay, mut subscription) = Relay::start(&pool).await;
        let database = Database::new(pool.clone());
        let workspace_id = database
            .repository::<TestRepository>()
            .create_workspace()
            .await
            .unwrap();
        let gallery_id = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap()
            .id;
        let photo_id = PhotoId::from(Uuid::new_v4());

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height)
             VALUES ($1, $2, $3, 'a.jpg', 1, 1, 1)",
        )
        .bind(photo_id)
        .bind(workspace_id)
        .bind(gallery_id)
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            recv(&mut subscription).await,
            Broadcast::Photo(PhotoEvent {
                workspace_id,
                gallery_id,
                photo_id,
                kind: PhotoKind::Proof,
                status: PhotoChange::Waiting,
            })
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn the_relay_publishes_committed_favorite_changes(pool: PgPool) {
        let (_relay, mut subscription) = Relay::start(&pool).await;
        let database = Database::new(pool.clone());
        let workspace_id = database
            .repository::<TestRepository>()
            .create_workspace()
            .await
            .unwrap();
        let gallery_id = database
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap()
            .id;

        sqlx::query(
            "INSERT INTO photos (id, workspace_id, gallery_id, original_name, size, width, height)
             VALUES (gen_random_uuid(), $1, $2, 'a.jpg', 1, 1, 1)",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("UPDATE photos SET favorite = true")
            .execute(&pool)
            .await
            .unwrap();

        loop {
            if let Broadcast::Favorites(event) = recv(&mut subscription).await {
                assert_eq!(
                    event,
                    FavoriteEvent {
                        workspace_id,
                        gallery_id
                    }
                );

                break;
            }
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn the_relay_publishes_export_changes_and_resyncs_on_an_undecodable_payload(
        pool: PgPool,
    ) {
        let (_relay, mut subscription) = Relay::start(&pool).await;
        let workspace_id = WorkspaceId(Uuid::new_v4());
        let gallery_id = GalleryId(Uuid::new_v4());
        let payload = json!({"workspace_id": workspace_id, "gallery_id": gallery_id});

        for payload in [payload.to_string(), "not json".to_owned()] {
            sqlx::query("SELECT pg_notify('export_events', $1)")
                .bind(payload)
                .execute(&pool)
                .await
                .unwrap();
        }

        assert_eq!(
            recv(&mut subscription).await,
            Broadcast::Export(ExportEvent {
                workspace_id,
                gallery_id
            })
        );
        assert_eq!(recv(&mut subscription).await, Broadcast::Resync);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn the_relay_holds_a_single_listener_connection(pool: PgPool) {
        let (_relay, _subscription) = Relay::start(&pool).await;
        let listeners: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
             WHERE datname = current_database() AND query LIKE 'LISTEN%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(listeners, 1);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_lost_listener_connection_is_followed_by_a_resync(pool: PgPool) {
        let (relay, mut subscription) = Relay::start(&pool).await;

        sqlx::query(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity
             WHERE datname = current_database() AND query LIKE 'LISTEN%'
               AND pid <> pg_backend_pid()",
        )
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(recv(&mut subscription).await, Broadcast::Resync);
        assert!(!relay.task.is_finished());
    }
}
