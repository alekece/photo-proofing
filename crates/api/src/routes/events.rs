//! `GET /api/v1/events`: the signed-in workspace's live photo changes as Server-Sent Events.
//!
//! A stream starts with a `resync` event, then sends `photo` events
//! (`{gallery_id, photo_id, status}`) for its own workspace only, and `resync` again whenever
//! events may have been lost. It holds the session read-only: on every keep-alive tick it checks
//! the session is still valid, never extending it, and ends once it is not.

use std::{
    pin::Pin,
    time::{Duration, SystemTime},
};

use actix_web::{Responder, get, web::Data};
use actix_web_lab::sse::{self, Sse};
use futures_util::{
    future,
    stream::{self, Stream, StreamExt},
};
use pp_core::notifications::PhotoId;
use pp_database::{
    auth::{AuthRepository, WorkspaceId},
    gallery::GalleryId,
    photo_events::{PhotoChange, PhotoEvent},
};
use serde::Serialize;
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

/// What a `photo` event tells the browser: which photo changed and its new status.
#[derive(Serialize)]
struct PhotoData {
    gallery_id: GalleryId,
    photo_id: PhotoId,
    status: PhotoChange,
}

#[get("/events")]
pub(crate) async fn stream_events(
    Authenticated(identity): Authenticated,
    token: SessionToken,
    state: Data<ApiState>,
    hub: Data<EventHub>,
) -> Result<impl Responder, ErrorResponse> {
    let workspace_id = identity.workspace_id;
    let subscription = hub
        .subscribe(workspace_id)
        .ok_or_else(ErrorResponse::unavailable)?;
    let keep_alive = hub.keep_alive();

    tracing::info!(workspace_id = %workspace_id.0, "event stream opened");

    let events = EventStream {
        subscription,
        state,
        token: token.digest(),
        workspace_id,
        session_check: session_check(keep_alive),
    };

    Ok(Sse::from_infallible_stream(events.into_stream()).with_keep_alive(keep_alive))
}

fn session_check(period: Duration) -> Interval {
    let mut interval = time::interval_at(Instant::now() + period, period);

    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    interval
}

/// One open stream; dropping it (the browser left) frees its hub slot.
struct EventStream {
    subscription: Subscription,
    state: Data<ApiState>,
    token: TokenHash,
    workspace_id: WorkspaceId,
    session_check: Interval,
}

impl EventStream {
    /// A `resync` first, since the browser cannot know what changed before it connected.
    fn into_stream(self) -> Pin<Box<dyn Stream<Item = sse::Event>>> {
        let following = stream::unfold(self, |mut events| async move {
            let event = events.next().await?;

            Some((event, events))
        });

        Box::pin(stream::once(future::ready(resync())).chain(following))
    }

    /// The next event to send, or `None` once the stream must end.
    async fn next(&mut self) -> Option<sse::Event> {
        loop {
            tokio::select! {
                received = self.subscription.receiver.recv() => match received {
                    Ok(Broadcast::Photo(event)) if event.workspace_id == self.workspace_id => {
                        return Some(photo(event));
                    }
                    Ok(Broadcast::Photo(_)) => {}
                    Ok(Broadcast::Resync) | Err(RecvError::Lagged(_)) => return Some(resync()),
                    Err(RecvError::Closed) => return None,
                },
                () = self.subscription.closing.cancelled() => return None,
                _ = self.session_check.tick() => {
                    if !self.session_is_valid().await {
                        return None;
                    }
                }
            }
        }
    }

    /// A database failure keeps the stream: it says nothing about the session, and the next
    /// tick checks again.
    async fn session_is_valid(&self) -> bool {
        let identity = self
            .state
            .database()
            .repository::<AuthRepository>()
            .authenticate(self.token.as_bytes(), SystemTime::now())
            .await;

        !matches!(identity, Ok(None))
    }
}

impl Drop for EventStream {
    fn drop(&mut self) {
        tracing::info!(workspace_id = %self.workspace_id.0, "event stream closed");
    }
}

fn resync() -> sse::Event {
    sse::Data::new("{}").event("resync").into()
}

fn photo(event: PhotoEvent) -> sse::Event {
    let data = PhotoData {
        gallery_id: event.gallery_id,
        photo_id: event.photo_id,
        status: event.status,
    };

    // Ids and a status always serialize; were that to change, a refetch is still correct.
    sse::Data::new_json(data).map_or_else(|_| resync(), |data| data.event("photo").into())
}

#[cfg(test)]
mod tests {
    use std::{future::poll_fn, sync::Arc};

    use actix_web::{
        App,
        body::{BoxBody, MessageBody},
        cookie::Cookie,
        http::StatusCode,
        test,
    };
    use pp_database::{Database, tests::TestRepository};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        api::configure_routes,
        events::HUB_CAPACITY,
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
        EventHub::new(&events_config(8, 8), HUB_CAPACITY)
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
                json!({"gallery_id": own.gallery_id, "photo_id": own.photo_id, "status": "ready"})
            )
        );

        client.hub.publish(Broadcast::Resync);

        assert_eq!(next_event(&mut body).await.0, "resync");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_lagging_stream_is_told_to_resync(pool: PgPool) {
        let client = client(pool, EventHub::new(&events_config(8, 8), 1)).await;
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
    async fn streams_beyond_the_cap_are_refused_until_one_closes(pool: PgPool) {
        let client = client(pool, EventHub::new(&events_config(1, 1), HUB_CAPACITY)).await;
        let cookie = sign_up(&client.app).await;
        let first = test::call_service(&client.app, open(&cookie).to_request()).await;

        assert_eq!(first.status(), StatusCode::OK);

        let (status, _) = call(&client.app, open(&cookie)).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

        drop(first);

        let second = test::call_service(&client.app, open(&cookie).to_request()).await;

        assert_eq!(second.status(), StatusCode::OK);
    }
}
