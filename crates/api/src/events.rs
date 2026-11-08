//! Live photo and favorite events, from the database to every open event stream.
//!
//! One [`EventHub::relay`] task per process listens to the database and publishes on the hub;
//! each `GET /api/v1/events` stream subscribes and forwards its own workspace's events (see
//! [`routes::events`](crate::routes::events)). Whenever events may have been lost (a reconnect,
//! an undecodable notification or a lagging subscriber), streams receive [`Broadcast::Resync`]
//! so the browser refetches instead of trusting a partial history.

use std::{
    collections::HashMap,
    num::NonZeroUsize,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    task::{Context, Poll},
    time::Duration,
};

use futures_util::Stream;
use pp_database::{
    Database,
    auth::WorkspaceId,
    error::DatabaseError,
    export::ExportEvent,
    favorite_events::FavoriteEvent,
    listener::{Channel, Listener, MissedReason, Notification},
    photo_events::PhotoEvent,
};
use tokio::{
    sync::broadcast,
    time::{self, Instant},
};
use tokio_util::sync::{CancellationToken, DropGuard};

use crate::config::{EventsConfig, KeepAlive, StallIntervals};

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

/// Fans database events out to the open streams, and bounds how many may be open.
pub(crate) struct EventHub {
    sender: broadcast::Sender<Broadcast>,
    slots: Arc<StreamSlots>,
    cancellation_token: CancellationToken,
    keep_alive: KeepAlive,
    stall_intervals: StallIntervals,
    relay_backoff_start: Duration,
    relay_backoff_cap: Duration,
}

impl EventHub {
    pub fn new(config: &EventsConfig) -> Self {
        let (sender, _) = broadcast::channel(usize::from(config.capacity.get()));

        Self {
            sender,
            slots: Arc::new(StreamSlots {
                counts: Mutex::default(),
                max_streams: config.max_streams,
                max_streams_per_workspace: config.max_streams_per_workspace,
            }),
            cancellation_token: CancellationToken::new(),
            keep_alive: config.keep_alive,
            stall_intervals: config.stall_intervals,
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

    /// Opens a subscription for a stream of `workspace_id`, or `None` when the process or the
    /// workspace already has as many streams as allowed.
    pub fn subscribe(&self, workspace_id: WorkspaceId) -> Option<Subscription> {
        let slot = self.slots.acquire(workspace_id)?;

        Some(Subscription {
            receiver: self.sender.subscribe(),
            cancellation_token: self.cancellation_token.clone(),
            lease: SlotLease(Arc::new(Mutex::new(Some(slot)))),
        })
    }

    /// Ends `stream` and gives back its slot when the client stops accepting data for
    /// `stall_intervals` keep-alive periods. The watchdog checks once per period, so a stall is
    /// noticed within one more.
    ///
    /// The server only polls a response body while the client reads it, and an idle stream is
    /// polled at least once per keep-alive period, so a longer silence means a stalled client.
    /// The watchdog task ends with the stream: dropping it cancels the task's token.
    pub fn watch<S: Stream + Unpin>(&self, stream: S, lease: SlotLease) -> StallWatch<S> {
        let last_poll = Arc::new(Mutex::new(Instant::now()));
        let stalled = CancellationToken::new();
        let finished = self.cancellation_token.child_token();

        tokio::spawn(
            StallWatchdog {
                period: self.keep_alive(),
                limit: self.keep_alive() * self.stall_intervals.get(),
                last_poll: Arc::clone(&last_poll),
                stalled: stalled.clone(),
                lease,
                finished: finished.clone(),
            }
            .run(),
        );

        StallWatch {
            inner: stream,
            last_poll,
            stalled,
            _finished: finished.drop_guard(),
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

    /// Listens to the database for the life of the process and publishes every photo and
    /// favorite change.
    ///
    /// Failures are retried with a growing delay; they never end the task, so the owner aborts
    /// it on shutdown.
    pub async fn relay(&self, database: Database) {
        tokio::join!(
            self.relay_channel::<PhotoEvent>(&database, Broadcast::Photo),
            self.relay_channel::<FavoriteEvent>(&database, Broadcast::Favorites),
            self.relay_channel::<ExportEvent>(&database, Broadcast::Export),
        );
    }

    async fn relay_channel<T: Channel>(&self, database: &Database, broadcast: fn(T) -> Broadcast) {
        let mut backoff = self.relay_backoff_start;

        loop {
            match database.listen::<T>().await {
                Ok(listener) => {
                    backoff = self.relay_backoff_start;

                    let error = self.forward(listener, broadcast).await;

                    tracing::warn!(%error, channel = T::NAME, "event listener failed");
                }
                Err(error) => {
                    tracing::warn!(%error, channel = T::NAME, "event listener unavailable");
                }
            }

            time::sleep(backoff).await;
            backoff = (backoff * 2).min(self.relay_backoff_cap);
        }
    }

    /// Publishes `listener`'s notifications until it fails. It starts with a resync, since
    /// changes made while no listener was open were not announced.
    async fn forward<T: Channel>(
        &self,
        mut listener: Listener<T>,
        broadcast: fn(T) -> Broadcast,
    ) -> DatabaseError {
        self.publish(Broadcast::Resync);

        loop {
            match listener.next().await {
                Ok(Notification::Event(event)) => self.publish(broadcast(event)),
                Ok(Notification::Missed(reason)) => {
                    if reason == MissedReason::Undecodable {
                        tracing::warn!(channel = T::NAME, "undecodable event notification");
                    }

                    self.publish(Broadcast::Resync);
                }
                Err(error) => return error,
            }
        }
    }
}

/// A stream's receiver and the slot it holds; dropping it frees the slot.
pub(crate) struct Subscription {
    pub receiver: broadcast::Receiver<Broadcast>,
    /// Cancelled when the hub closes: the stream must end so the server can stop.
    pub cancellation_token: CancellationToken,
    lease: SlotLease,
}

impl Subscription {
    /// The slot's lease, for the watchdog that frees it if the client stalls.
    pub fn lease(&self) -> SlotLease {
        self.lease.clone()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.lease.release();
    }
}

/// A stream slot that either its stream or the stall watchdog gives back, whichever is first.
#[derive(Clone)]
pub(crate) struct SlotLease(Arc<Mutex<Option<StreamSlot>>>);

impl SlotLease {
    fn release(&self) {
        let slot = self.0.lock().unwrap_or_else(PoisonError::into_inner).take();

        drop(slot);
    }
}

/// A stream that ends once its stall watchdog fires; see [`EventHub::watch`].
pub(crate) struct StallWatch<S> {
    inner: S,
    last_poll: Arc<Mutex<Instant>>,
    stalled: CancellationToken,
    /// Stops the watchdog when the stream is dropped.
    _finished: DropGuard,
}

impl<S: Stream + Unpin> Stream for StallWatch<S> {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        *self
            .last_poll
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Instant::now();

        if self.stalled.is_cancelled() {
            return Poll::Ready(None);
        }

        Pin::new(&mut self.inner).poll_next(context)
    }
}

/// Checks every `period` whether a stream has gone unpolled for `limit`, and if so ends it and
/// frees its slot.
struct StallWatchdog {
    period: Duration,
    limit: Duration,
    last_poll: Arc<Mutex<Instant>>,
    stalled: CancellationToken,
    lease: SlotLease,
    finished: CancellationToken,
}

impl StallWatchdog {
    async fn run(self) {
        loop {
            if self
                .finished
                .run_until_cancelled(time::sleep(self.period))
                .await
                .is_none()
            {
                return;
            }

            let silence = self
                .last_poll
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .elapsed();

            if silence >= self.limit {
                tracing::warn!(?silence, "event stream stalled; closing it");
                self.stalled.cancel();
                self.lease.release();

                return;
            }
        }
    }
}

/// How many streams are open, overall and per workspace, against their caps.
struct StreamSlots {
    counts: Mutex<StreamCounts>,
    max_streams: NonZeroUsize,
    max_streams_per_workspace: NonZeroUsize,
}

impl StreamSlots {
    fn acquire(self: &Arc<Self>, workspace_id: WorkspaceId) -> Option<StreamSlot> {
        let mut counts = self.counts();
        let open = counts.workspaces.get(&workspace_id).copied().unwrap_or(0);

        if counts.total >= self.max_streams.get() || open >= self.max_streams_per_workspace.get() {
            return None;
        }

        counts.total += 1;
        counts.workspaces.insert(workspace_id, open + 1);

        Some(StreamSlot {
            slots: Arc::clone(self),
            workspace_id,
        })
    }

    /// The counts stay consistent even if a holder panicked: every update is a single step.
    fn counts(&self) -> MutexGuard<'_, StreamCounts> {
        self.counts.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[derive(Default)]
struct StreamCounts {
    total: usize,
    workspaces: HashMap<WorkspaceId, usize>,
}

/// One open stream's place in [`StreamSlots`]; dropping it frees the place.
struct StreamSlot {
    slots: Arc<StreamSlots>,
    workspace_id: WorkspaceId,
}

impl Drop for StreamSlot {
    fn drop(&mut self) {
        let mut counts = self.slots.counts();

        counts.total -= 1;

        if let Some(open) = counts.workspaces.get_mut(&self.workspace_id) {
            *open -= 1;

            if *open == 0 {
                counts.workspaces.remove(&self.workspace_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use pp_database::photo::PhotoId;
    use pp_database::{
        gallery::GalleryRepository, photo::PhotoKind, photo_events::PhotoChange,
        tests::TestRepository,
    };
    use sqlx::PgPool;
    use tokio::task::JoinHandle;
    use uuid::Uuid;

    use super::*;
    use crate::tests::events_config;

    fn workspace() -> WorkspaceId {
        WorkspaceId(Uuid::new_v4())
    }

    async fn recv(subscription: &mut Subscription) -> Broadcast {
        time::timeout(Duration::from_secs(5), subscription.receiver.recv())
            .await
            .expect("a broadcast arrives")
            .unwrap()
    }

    #[test]
    fn subscriptions_are_capped_overall_and_per_workspace_until_dropped() {
        let hub = EventHub::new(&events_config(3, 2));
        let (first, second, third) = (workspace(), workspace(), workspace());
        let a = hub.subscribe(first).unwrap();
        let _b = hub.subscribe(first).unwrap();

        assert!(hub.subscribe(first).is_none(), "workspace cap");

        let _c = hub.subscribe(second).unwrap();

        assert!(hub.subscribe(third).is_none(), "overall cap");

        drop(a);

        assert!(hub.subscribe(third).is_some());
    }

    #[test]
    fn a_workspace_without_streams_is_forgotten() {
        let hub = EventHub::new(&events_config(3, 2));
        let subscription = hub.subscribe(workspace()).unwrap();

        drop(subscription);

        let counts = hub.slots.counts();

        assert_eq!(counts.total, 0);
        assert!(counts.workspaces.is_empty());
    }

    struct Relay {
        task: JoinHandle<()>,
    }

    impl Relay {
        /// Starts a relay and waits until its listener is open, signalled by the first resync.
        async fn start(pool: &PgPool) -> (Self, Subscription) {
            let hub = Arc::new(EventHub::new(&events_config(8, 8)));
            let mut subscription = hub.subscribe(workspace()).unwrap();
            let database = Database::new(pool.clone());
            let task = tokio::spawn({
                let hub = Arc::clone(&hub);

                async move { hub.relay(database).await }
            });

            // One initial resync per listened channel.
            for _ in 0..3 {
                assert_eq!(recv(&mut subscription).await, Broadcast::Resync);
            }

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
