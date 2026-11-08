//! Live changes as Server-Sent Events: `GET /api/v1/events` for the signed-in workspace.
//!
//! A stream starts with a `resync` event, then sends `photo` events
//! (`{workspace_id, gallery_id, photo_id, kind, status}`) and `favorites` events (`{workspace_id,
//! gallery_id}`, a client changed a selection) and `export` events (same shape: a selection ZIP
//! changed status) for its own workspace only, and `resync` again
//! whenever events may have been lost. It holds the session read-only: on every keep-alive tick it
//! checks the session is still valid, never extending it, and ends once it is not. Clients have no
//! stream.

use std::time::SystemTime;

use actix_web::{Responder, get, web::Data};
use actix_web_lab::sse::{self, Sse};
use futures_util::{
    future,
    stream::{self, Stream, StreamExt},
};
use pp_database::auth::{AuthRepository, WorkspaceId};
use tokio::{
    sync::broadcast::error::RecvError,
    time::{self, Instant, Interval, MissedTickBehavior},
};

use crate::{
    api::ApiState,
    error::ErrorResponse,
    events::{Broadcast, EventHub, Subscription},
    extractors::{
        identity::Authenticated,
        session_token::{SessionToken, TokenHash},
    },
};

/// The workspace a stream serves and the session that keeps it open.
struct Scope {
    token: TokenHash,
    workspace_id: WorkspaceId,
}

impl Scope {
    fn admits(&self, broadcast: &Broadcast) -> bool {
        match broadcast {
            Broadcast::Resync => true,
            Broadcast::Photo(event) => event.workspace_id == self.workspace_id,
            Broadcast::Favorites(event) => event.workspace_id == self.workspace_id,
            Broadcast::Export(event) => event.workspace_id == self.workspace_id,
        }
    }

    /// A database failure keeps the stream: it says nothing about the session, and the next tick
    /// checks again.
    async fn is_valid(&self, state: &ApiState) -> bool {
        !matches!(
            state
                .database()
                .repository::<AuthRepository>()
                .authenticate(self.token.as_bytes(), SystemTime::now())
                .await,
            Ok(None)
        )
    }
}

/// One open stream; dropping it (the browser left) frees its hub slot.
struct EventStream {
    subscription: Subscription,
    state: Data<ApiState>,
    scope: Scope,
    session_check: Interval,
}

impl EventStream {
    fn open(
        state: Data<ApiState>,
        hub: &EventHub,
        scope: Scope,
    ) -> Result<impl Responder + use<>, ErrorResponse> {
        let subscription = hub
            .subscribe(scope.workspace_id)
            .ok_or_else(ErrorResponse::unavailable)?;
        let keep_alive = hub.keep_alive();
        let mut session_check = time::interval_at(Instant::now() + keep_alive, keep_alive);

        session_check.set_missed_tick_behavior(MissedTickBehavior::Delay);
        tracing::info!(workspace = %scope.workspace_id.0, "event stream opened");

        let events = EventStream {
            subscription,
            state,
            scope,
            session_check,
        };

        Ok(Sse::from_infallible_stream(events.into_stream(hub)).with_keep_alive(keep_alive))
    }

    /// A `resync` first, since the browser cannot know what changed before it connected.
    fn into_stream(self, hub: &EventHub) -> impl Stream<Item = sse::Event> + use<> {
        let lease = self.subscription.lease();
        let following = stream::unfold(self, |mut events| async move {
            let broadcast = events.next().await?;

            Some((broadcast, events))
        });

        let events = Box::pin(
            stream::once(future::ready(Broadcast::Resync))
                .chain(following)
                .map(sse::Event::from),
        );

        hub.watch(events, lease)
    }

    /// The next broadcast to send, or `None` once the stream must end.
    async fn next(&mut self) -> Option<Broadcast> {
        loop {
            tokio::select! {
                received = self.subscription.receiver.recv() => match received {
                    Ok(broadcast) if !self.scope.admits(&broadcast) => {}
                    Ok(broadcast) => return Some(broadcast),
                    Err(RecvError::Lagged(_)) => return Some(Broadcast::Resync),
                    Err(RecvError::Closed) => return None,
                },
                () = self.subscription.cancellation_token.cancelled() => return None,
                _ = self.session_check.tick() => {
                    let cancellation_token = &self.subscription.cancellation_token;

                    // A slow check must not hold the stream open past shutdown.
                    if cancellation_token.run_until_cancelled(self.scope.is_valid(&self.state))
                        .await != Some(true)
                    {
                        return None;
                    }
                }
            }
        }
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        tracing::info!(workspace = %self.scope.workspace_id.0, "event stream closed");
    }
}

impl From<Broadcast> for sse::Event {
    fn from(broadcast: Broadcast) -> Self {
        let resync = || sse::Data::new("{}").event("resync").into();

        match broadcast {
            // Ids and a status always serialize; were that to change, a refetch is still correct.
            Broadcast::Photo(event) => sse::Data::new_json(event)
                .map_or_else(|_| resync(), |data| data.event("photo").into()),
            Broadcast::Favorites(event) => sse::Data::new_json(event)
                .map_or_else(|_| resync(), |data| data.event("favorites").into()),
            Broadcast::Export(event) => sse::Data::new_json(event)
                .map_or_else(|_| resync(), |data| data.event("export").into()),
            Broadcast::Resync => resync(),
        }
    }
}

#[get("/events")]
pub(crate) async fn stream_events(
    Authenticated(identity): Authenticated,
    token: SessionToken,
    state: Data<ApiState>,
    hub: Data<EventHub>,
) -> Result<impl Responder, ErrorResponse> {
    let workspace_id = identity.workspace_id;
    let scope = Scope {
        token: token.digest(),
        workspace_id,
    };

    EventStream::open(state, &hub, scope)
}

#[cfg(test)]
mod tests {
    use std::{future::poll_fn, num::NonZeroU16, pin::Pin, sync::Arc, time::Duration};

    use actix_web::{
        App,
        body::{BoxBody, MessageBody},
        cookie::Cookie,
        http::StatusCode,
        test,
    };
    use pp_database::photo::PhotoId;
    use pp_database::{
        Database,
        favorite_events::FavoriteEvent,
        gallery::GalleryId,
        photo::PhotoKind,
        photo_events::{PhotoChange, PhotoEvent},
        tests::TestRepository,
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        api::configure_routes,
        config::EventsConfig,
        tests::{ORIGIN, app_state, call, events_config, sign_up},
    };

    struct Client<S> {
        app: S,
        hub: Arc<EventHub>,
        pool: PgPool,
        _directory: tempfile::TempDir,
    }

    async fn client(
        pool: PgPool,
        hub: EventHub,
    ) -> Client<
        impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
    > {
        let (state, directory) = app_state(pool.clone()).await;
        let hub = Arc::new(hub);
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .app_data(Data::from(Arc::clone(&hub)))
                .configure(configure_routes),
        )
        .await;

        Client {
            app,
            hub,
            pool,
            _directory: directory,
        }
    }

    fn default_hub() -> EventHub {
        EventHub::new(&events_config(8, 8))
    }

    fn open(cookie: &Cookie<'static>) -> test::TestRequest {
        test::TestRequest::get()
            .uri("/api/v1/events")
            .cookie(cookie.clone())
    }

    async fn next_chunk(body: &mut BoxBody) -> Option<String> {
        let chunk = time::timeout(
            Duration::from_secs(5),
            poll_fn(|context| Pin::new(&mut *body).poll_next(context)),
        )
        .await
        .expect("the stream sends or ends in time")?;

        Some(String::from_utf8(chunk.unwrap().to_vec()).unwrap())
    }

    /// The next event's name and data, skipping keep-alive comments.
    async fn next_event(body: &mut BoxBody) -> (String, Value) {
        loop {
            let chunk = next_chunk(body).await.expect("an event arrives");
            let mut name = None;
            let mut data = Value::Null;

            for line in chunk.lines() {
                if let Some(value) = line.strip_prefix("event: ") {
                    name = Some(value.to_owned());
                } else if let Some(value) = line.strip_prefix("data: ") {
                    data = serde_json::from_str(value).unwrap();
                }
            }

            if let Some(name) = name {
                return (name, data);
            }
        }
    }

    async fn workspace_of<S>(app: &S, cookie: &Cookie<'static>) -> WorkspaceId
    where
        S: actix_web::dev::Service<
                actix_http::Request,
                Response = actix_web::dev::ServiceResponse,
                Error = actix_web::Error,
            >,
    {
        let request = test::TestRequest::get()
            .uri("/api/v1/auth/status")
            .cookie(cookie.clone());
        let (_, body) = call(app, request).await;

        serde_json::from_value(body["identity"]["workspace_id"].clone()).unwrap()
    }

    fn photo_event(workspace_id: WorkspaceId) -> PhotoEvent {
        PhotoEvent {
            workspace_id,
            gallery_id: GalleryId(Uuid::new_v4()),
            photo_id: PhotoId::from(Uuid::new_v4()),
            kind: PhotoKind::Proof,
            status: PhotoChange::Ready,
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_stream_needs_a_session(pool: PgPool) {
        let client = client(pool, default_hub()).await;
        let (status, _) = call(&client.app, test::TestRequest::get().uri("/api/v1/events")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_stream_starts_with_resync_then_sends_its_own_workspace_events(pool: PgPool) {
        let client = client(pool, default_hub()).await;
        let cookie = sign_up(&client.app).await;
        let workspace_id = workspace_of(&client.app, &cookie).await;
        let response = test::call_service(&client.app, open(&cookie).to_request()).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/event-stream"
        );

        let mut body = response.into_body();

        assert_eq!(next_event(&mut body).await.0, "resync");

        let own = photo_event(workspace_id);

        client
            .hub
            .publish(Broadcast::Photo(photo_event(WorkspaceId(Uuid::new_v4()))));
        client.hub.publish(Broadcast::Photo(own));

        assert_eq!(
            next_event(&mut body).await,
            (
                "photo".to_owned(),
                json!({
                    "workspace_id": own.workspace_id,
                    "gallery_id": own.gallery_id,
                    "photo_id": own.photo_id,
                    "kind": "proof",
                    "status": "ready",
                })
            )
        );

        client.hub.publish(Broadcast::Resync);

        assert_eq!(next_event(&mut body).await.0, "resync");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_stream_sends_only_its_own_workspace_favorites(pool: PgPool) {
        let client = client(pool, default_hub()).await;
        let cookie = sign_up(&client.app).await;
        let workspace_id = workspace_of(&client.app, &cookie).await;
        let gallery_id = GalleryId(Uuid::new_v4());
        let response = test::call_service(&client.app, open(&cookie).to_request()).await;
        let mut body = response.into_body();

        assert_eq!(next_event(&mut body).await.0, "resync");

        client.hub.publish(Broadcast::Favorites(FavoriteEvent {
            workspace_id: WorkspaceId(Uuid::new_v4()),
            gallery_id,
        }));
        client.hub.publish(Broadcast::Favorites(FavoriteEvent {
            workspace_id,
            gallery_id,
        }));

        assert_eq!(
            next_event(&mut body).await,
            (
                "favorites".to_owned(),
                json!({"workspace_id": workspace_id, "gallery_id": gallery_id})
            )
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_lagging_stream_is_told_to_resync(pool: PgPool) {
        let config = EventsConfig {
            capacity: NonZeroU16::new(1).unwrap(),
            ..events_config(8, 8)
        };
        let client = client(pool, EventHub::new(&config)).await;
        let cookie = sign_up(&client.app).await;
        let workspace_id = workspace_of(&client.app, &cookie).await;
        let response = test::call_service(&client.app, open(&cookie).to_request()).await;
        let mut body = response.into_body();

        assert_eq!(next_event(&mut body).await.0, "resync");

        for _ in 0..3 {
            client
                .hub
                .publish(Broadcast::Photo(photo_event(workspace_id)));
        }

        assert_eq!(next_event(&mut body).await.0, "resync");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn signing_out_ends_the_stream_without_the_stream_extending_the_session(pool: PgPool) {
        let client = client(pool, default_hub()).await;
        let cookie = sign_up(&client.app).await;
        let tests = Database::new(client.pool.clone());
        let expiry = tests
            .repository::<TestRepository>()
            .session_expiry()
            .await
            .unwrap();
        let response = test::call_service(&client.app, open(&cookie).to_request()).await;
        let mut body = response.into_body();

        assert_eq!(next_event(&mut body).await.0, "resync");

        time::sleep(Duration::from_millis(1500)).await;

        assert_eq!(
            tests
                .repository::<TestRepository>()
                .session_expiry()
                .await
                .unwrap(),
            expiry
        );

        let logout = test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone());
        let (status, _) = call(&client.app, logout).await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let ended = time::timeout(Duration::from_secs(5), async {
            while let Some(chunk) = next_chunk(&mut body).await {
                assert!(
                    chunk.starts_with(':'),
                    "only keep-alives before the end: {chunk}"
                );
            }
        })
        .await;

        assert!(ended.is_ok(), "the stream ends after sign-out");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn closing_the_hub_ends_open_streams(pool: PgPool) {
        let client = client(pool, default_hub()).await;
        let cookie = sign_up(&client.app).await;
        let response = test::call_service(&client.app, open(&cookie).to_request()).await;
        let mut body = response.into_body();

        assert_eq!(next_event(&mut body).await.0, "resync");

        client.hub.close();

        assert_eq!(next_chunk(&mut body).await, None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn closing_the_hub_ends_a_stream_waiting_on_its_session_check(pool: PgPool) {
        let client = client(pool, default_hub()).await;
        let cookie = sign_up(&client.app).await;
        let response = test::call_service(&client.app, open(&cookie).to_request()).await;
        let mut body = response.into_body();

        assert_eq!(next_event(&mut body).await.0, "resync");

        let tests = Database::new(client.pool.clone());
        let sessions_lock = tests
            .repository::<TestRepository>()
            .lock_sessions()
            .await
            .unwrap();

        // Keep reading so the stream reaches its session check, which then waits on the lock.
        let _ = time::timeout(Duration::from_millis(1500), async {
            while next_chunk(&mut body).await.is_some() {}
        })
        .await;
        client.hub.close();

        let ended = time::timeout(Duration::from_secs(5), async {
            while next_chunk(&mut body).await.is_some() {}
        })
        .await;

        drop(sessions_lock);

        assert!(
            ended.is_ok(),
            "the stream ends while its session check waits"
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn streams_beyond_the_cap_are_refused_until_one_closes(pool: PgPool) {
        let client = client(pool, EventHub::new(&events_config(1, 1))).await;
        let cookie = sign_up(&client.app).await;
        let first = test::call_service(&client.app, open(&cookie).to_request()).await;

        assert_eq!(first.status(), StatusCode::OK);

        let (status, _) = call(&client.app, open(&cookie)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

        drop(first);

        let second = test::call_service(&client.app, open(&cookie).to_request()).await;

        assert_eq!(second.status(), StatusCode::OK);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_stream_the_client_stops_reading_is_closed_and_frees_its_slot(pool: PgPool) {
        let client = client(pool, EventHub::new(&events_config(1, 1))).await;
        let cookie = sign_up(&client.app).await;
        let first = test::call_service(&client.app, open(&cookie).to_request()).await;
        let mut body = first.into_body();

        next_event(&mut body).await;

        let deadline = time::Instant::now() + Duration::from_secs(10);
        let mut status = StatusCode::SERVICE_UNAVAILABLE;

        while status != StatusCode::OK && time::Instant::now() < deadline {
            time::sleep(Duration::from_millis(250)).await;
            status = test::call_service(&client.app, open(&cookie).to_request())
                .await
                .status();
        }

        assert_eq!(status, StatusCode::OK, "the stalled stream's slot is freed");
        assert_eq!(next_chunk(&mut body).await, None);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_stream_read_past_the_stall_bound_stays_open(pool: PgPool) {
        let client = client(pool, EventHub::new(&events_config(1, 1))).await;
        let cookie = sign_up(&client.app).await;
        let first = test::call_service(&client.app, open(&cookie).to_request()).await;
        let mut body = first.into_body();

        next_event(&mut body).await;

        for _ in 0..4 {
            assert!(next_chunk(&mut body).await.is_some());
        }

        let (status, _) = call(&client.app, open(&cookie)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }
}
