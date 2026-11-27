//! The workspace activity log, under `/api/v1`. The paths say `timeline`, not `activity`, because
//! browser blockers cancel requests whose path contains that word.
//!
//! - `GET /timeline` lists what clients did across the workspace's galleries, newest first,
//!   leaving out archived galleries.
//! - `GET /galleries/{id}/timeline` lists one gallery's lines, archived or not; a gallery of
//!   another workspace is a 404.
//!
//! Both take the optional filters `contact` (a contact id or `anonymous`), `kind`, `range`
//! (`today`, `7d` or `30d`), `sort` (`newest` or `oldest`) and `cursor`, which is the `next`
//! of the previous page. An exhausted oldest-first page offers its `tail` as a cursor for
//! checking for newer rows. Keep the same sort across pages. The workspace
//! feed also takes `gallery`. The log is the photographer's: it needs a session and names
//! contacts, which no client route does. A change arrives on the event stream as a
//! `notification` event.

use actix_web::{
    HttpResponse, get, post,
    web::{Data, Path, Query, ServiceConfig},
};
use pp_database::{
    activity::{
        ActivityCursor, ActivityEventId, ActivityKind, ActivityOrder, ActivityQuery, ActivityRange,
        ActivityRepository, ActivitySearch, ContactFilter,
    },
    gallery::{GalleryId, GalleryRepository},
};
use serde::Deserialize;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

/// Registers the activity routes. They sit beside the `/galleries` scope, ahead of it, which
/// would otherwise answer the per-gallery path with a 404.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(list_activity)
        .service(list_gallery_activity)
        .service(retry_email);
}

#[post("/galleries/{id}/timeline/{event}/retry")]
async fn retry_email(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: Path<(GalleryId, ActivityEventId)>,
) -> Result<HttpResponse, ErrorResponse> {
    let (gallery, event) = path.into_inner();
    let retried = state
        .database()
        .repository::<ActivityRepository>()
        .retry_email(identity.workspace_id, gallery, event)
        .await?;

    if !retried {
        return Err(ErrorResponse::not_found());
    }

    Ok(HttpResponse::NoContent().no_store().finish())
}

#[derive(Deserialize)]
struct ActivityFilters {
    gallery: Option<GalleryId>,
    contact: Option<ContactFilter>,
    kind: Option<ActivityKind>,
    range: Option<ActivityRange>,
    cursor: Option<ActivityCursor>,
    q: Option<String>,
    #[serde(default)]
    sort: ActivityOrder,
}

impl ActivityFilters {
    fn query(&self, scoped_gallery: Option<GalleryId>) -> ActivityQuery<'_> {
        let search = self.q.as_deref().map(str::trim).filter(|q| !q.is_empty());

        ActivityQuery {
            gallery: scoped_gallery.or(self.gallery),
            contact: self.contact,
            kind: self.kind,
            range: self.range,
            cursor: self.cursor,
            sort: self.sort,
            include_archived: scoped_gallery.is_some(),
            search: search.map(|q| match scoped_gallery {
                Some(_) => ActivitySearch::Visitors(q),
                None => ActivitySearch::GalleriesAndVisitors(q),
            }),
        }
    }
}

#[get("/timeline")]
async fn list_activity(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    filters: Query<ActivityFilters>,
) -> Result<HttpResponse, ErrorResponse> {
    let page = state
        .database()
        .repository::<ActivityRepository>()
        .list(identity.workspace_id, filters.query(None))
        .await?;

    Ok(HttpResponse::Ok().no_store().json(page))
}

#[get("/galleries/{id}/timeline")]
async fn list_gallery_activity(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
    filters: Query<ActivityFilters>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();

    state
        .database()
        .repository::<GalleryRepository>()
        .get(identity.workspace_id, gallery_id)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    let page = state
        .database()
        .repository::<ActivityRepository>()
        .list(identity.workspace_id, filters.query(Some(gallery_id)))
        .await?;

    Ok(HttpResponse::Ok().no_store().json(page))
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU16, time::Duration};

    use actix_web::{App, http::StatusCode, test, web::Data};
    use pp_database::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository, InvitationBatch},
        jobs::{JobKind, JobOptions, JobRepository},
    };
    use serde_json::Value;
    use sqlx::PgPool;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

    /// The gallery a `POST /api/v1/galleries` response named, with its workspace.
    async fn gallery_of(pool: &PgPool, gallery: &Value) -> (WorkspaceId, GalleryId) {
        let gallery_id = GalleryId(gallery["id"].as_str().unwrap().parse().unwrap());
        let workspace_id = sqlx::query_scalar("SELECT workspace_id FROM galleries WHERE id = $1")
            .bind(gallery_id)
            .fetch_one(pool)
            .await
            .unwrap();

        (workspace_id, gallery_id)
    }

    async fn insert_line(
        pool: &PgPool,
        (workspace_id, gallery_id): (WorkspaceId, GalleryId),
        kind: &str,
        minutes_ago: i32,
    ) {
        sqlx::query(
            "INSERT INTO activity_events (workspace_id, gallery_id, kind, count, last_at)
             VALUES ($1, $2, $3, 1, now() - make_interval(mins => $4))",
        )
        .bind(workspace_id)
        .bind(gallery_id)
        .bind(kind)
        .bind(minutes_ago)
        .execute(pool)
        .await
        .unwrap();
    }

    fn get(uri: &str, cookie: &actix_web::cookie::Cookie<'static>) -> test::TestRequest {
        test::TestRequest::get()
            .uri(uri)
            .cookie(cookie.clone())
            .insert_header(("Origin", ORIGIN))
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn name_search_matches_older_events_before_pagination(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let create = || {
            test::TestRequest::post()
                .uri("/api/v1/galleries")
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN))
        };
        let (_, target) = call(&app, create()).await;
        let (_, recent) = call(&app, create()).await;
        let (workspace, target_id) = gallery_of(&pool, &target).await;
        let (_, recent_id) = gallery_of(&pool, &recent).await;

        sqlx::query("UPDATE galleries SET title = 'Été 100%' WHERE id = $1")
            .bind(target_id)
            .execute(&pool)
            .await
            .unwrap();
        insert_line(&pool, (workspace, target_id), "open", 120).await;
        sqlx::query(
            "INSERT INTO activity_events (workspace_id, gallery_id, kind, count, last_at)
             SELECT $1, $2, 'open', 1, now() - make_interval(mins => minute)
             FROM generate_series(1, 100) AS minute",
        )
        .bind(workspace)
        .bind(recent_id)
        .execute(&pool)
        .await
        .unwrap();

        let (_, initial) = call(&app, get("/api/v1/timeline", &cookie)).await;
        assert!(initial["next"].is_string());
        assert!(
            initial["events"]
                .as_array()
                .unwrap()
                .iter()
                .all(|event| event["gallery_id"] == recent["id"])
        );

        let (status, found) = call(
            &app,
            get(
                "/api/v1/timeline?q=%20ETE%20100%25%20&kind=open&range=today",
                &cookie,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(found["events"].as_array().unwrap().len(), 1);
        assert_eq!(found["events"][0]["gallery_id"], target["id"]);

        let (_, literal) = call(&app, get("/api/v1/timeline?q=E_E", &cookie)).await;
        assert!(literal["events"].as_array().unwrap().is_empty());
        let (_, blank) = call(&app, get("/api/v1/timeline?q=%20", &cookie)).await;
        assert_eq!(blank, initial);
        let (_, wrong_kind) = call(&app, get("/api/v1/timeline?q=ete&kind=comment", &cookie)).await;
        assert!(wrong_kind["events"].as_array().unwrap().is_empty());

        let local = format!("/api/v1/galleries/{}/timeline", target_id.0);
        let (_, gallery_title) = call(&app, get(&format!("{local}?q=ete"), &cookie)).await;
        assert!(gallery_title["events"].as_array().unwrap().is_empty());
        let (_, anonymous) = call(&app, get(&format!("{local}?q=anonyme"), &cookie)).await;
        assert_eq!(anonymous["events"].as_array().unwrap().len(), 1);

        sqlx::query(
            "WITH visitor AS (
                INSERT INTO contacts (id, workspace_id, name, email)
                VALUES (gen_random_uuid(), $1, 'Élodie', 'elodie@example.test')
                RETURNING id
             )
             UPDATE activity_events SET contact_id = visitor.id
             FROM visitor WHERE gallery_id = $2",
        )
        .bind(workspace)
        .bind(target_id)
        .execute(&pool)
        .await
        .unwrap();

        let (_, named) = call(&app, get(&format!("{local}?q=ELO"), &cookie)).await;
        assert_eq!(named["events"][0]["contact"]["name"], "Élodie");
        let (_, named) = call(&app, get("/api/v1/timeline?q=elo", &cookie)).await;
        assert_eq!(named["events"].as_array().unwrap().len(), 1);
        assert_eq!(named["events"][0]["gallery_id"], target["id"]);
        let (_, anonymous) = call(&app, get(&format!("{local}?q=anonyme"), &cookie)).await;
        assert!(anonymous["events"].as_array().unwrap().is_empty());
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn only_an_eligible_failed_email_can_be_retried_from_its_activity_line(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let post = |uri: &str| {
            test::TestRequest::post()
                .uri(uri)
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN))
        };
        let (_, gallery) = call(&app, post("/api/v1/galleries")).await;
        let (workspace, id) = gallery_of(&pool, &gallery).await;
        let database = Database::new(pool.clone());
        let galleries = database.repository::<GalleryRepository>();
        let jobs = database.repository::<JobRepository>();
        let batch = InvitationBatch::new(
            vec![],
            vec![
                "first@example.test".parse().unwrap(),
                "second@example.test".parse().unwrap(),
            ],
            "Private subject".parse().unwrap(),
            "Private text".parse().unwrap(),
        );
        let options = JobOptions {
            max_attempts: NonZeroU16::MIN,
            ..JobOptions::default()
        };
        galleries
            .queue_invitations(workspace, id, &batch, options)
            .await
            .unwrap()
            .unwrap();
        let first = jobs
            .claim(&[JobKind::SendEmail], Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        assert!(jobs.complete(&first).await.unwrap().is_current());
        let second = jobs
            .claim(&[JobKind::SendEmail], Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        assert!(
            jobs.claim(&[JobKind::SendEmail], Duration::ZERO)
                .await
                .unwrap()
                .is_none()
        );
        let timeline = format!("/api/v1/galleries/{}/timeline", id.0);
        let (_, feed) = call(&app, get("/api/v1/timeline", &cookie)).await;
        let (_, local) = call(&app, get(&timeline, &cookie)).await;
        assert_eq!(feed, local);
        let events = feed["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event["kind"], "email_failed");
        assert_eq!(event["can_retry"], true);
        assert!(event["contact"].is_null() && event["detail"].is_null());
        assert!(!event.to_string().contains("example.test"));
        let event_id = event["id"].as_str().unwrap();
        let retry = format!("{timeline}/{event_id}/retry");
        let foreign = galleries
            .create(other_workspace(&pool).await)
            .await
            .unwrap();
        assert_eq!(
            call(
                &app,
                post(&format!(
                    "/api/v1/galleries/{}/timeline/{event_id}/retry",
                    foreign.id.0
                ))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );

        sqlx::query(
            "UPDATE galleries SET expires_at = (now() AT TIME ZONE 'UTC')::date - 1 WHERE id=$1",
        )
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
        assert_eq!(call(&app, post(&retry)).await.0, StatusCode::NOT_FOUND);
        let (_, expired) = call(&app, get(&timeline, &cookie)).await;
        assert_eq!(expired["events"][0]["can_retry"], false);
        sqlx::query("UPDATE galleries SET expires_at=NULL WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        let (a, b) = futures_util::join!(call(&app, post(&retry)), call(&app, post(&retry)));
        assert!(matches!(
            (a.0, b.0),
            (StatusCode::NO_CONTENT, StatusCode::NOT_FOUND)
                | (StatusCode::NOT_FOUND, StatusCode::NO_CONTENT)
        ));
        let retried = jobs
            .claim(&[JobKind::SendEmail], Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retried.id, second.id);
        assert!(jobs.abandon(&retried, "Private error").await.unwrap());
        let (_, failed) = call(&app, get(&timeline, &cookie)).await;
        assert_eq!(failed["events"].as_array().unwrap().len(), 1);
        assert_eq!(failed["events"][0]["id"], event_id);
        assert_eq!(failed["events"][0]["can_retry"], true);
        assert!(!failed.to_string().contains("Private"));
        assert_eq!(call(&app, post(&retry)).await.0, StatusCode::NO_CONTENT);
        let last = jobs
            .claim(&[JobKind::SendEmail], Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        assert!(jobs.complete(&last).await.unwrap().is_current());
        let (_, historical) = call(&app, get(&timeline, &cookie)).await;
        assert_eq!(historical["events"][0]["id"], event_id);
        assert_eq!(historical["events"][0]["can_retry"], false);
        let bell: i64 = sqlx::query_scalar("SELECT count(*) FROM notifications")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(bell, 0);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn the_feed_needs_a_session_and_hides_archived_galleries_but_not_their_own_list(
        pool: PgPool,
    ) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, open) = call(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/galleries")
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN)),
        )
        .await;
        let (_, archived) = call(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/galleries")
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN)),
        )
        .await;
        let (open, archived) = (
            gallery_of(&pool, &open).await,
            gallery_of(&pool, &archived).await,
        );

        insert_line(&pool, open, "open", 5).await;
        insert_line(&pool, archived, "comment", 10).await;
        sqlx::query("UPDATE galleries SET archived_at = now() WHERE id = $1")
            .bind(archived.1)
            .execute(&pool)
            .await
            .unwrap();

        let (status, _) = call(&app, test::TestRequest::get().uri("/api/v1/timeline")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, feed) = call(&app, get("/api/v1/timeline", &cookie)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(feed["events"].as_array().unwrap().len(), 1);
        assert_eq!(feed["events"][0]["kind"], "open");
        assert_eq!(feed["events"][0]["contact"], Value::Null);
        assert_eq!(feed["next"], Value::Null);

        let (status, own) = call(
            &app,
            get(
                &format!("/api/v1/galleries/{}/timeline", archived.1.0),
                &cookie,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(own["events"][0]["kind"], "comment");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn filters_and_cursor_narrow_the_feed_and_bad_ones_are_refused(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, gallery) = call(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/galleries")
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN)),
        )
        .await;
        let gallery = gallery_of(&pool, &gallery).await;

        for minutes in 0..30 {
            insert_line(&pool, gallery, "open", minutes).await;
        }

        insert_line(&pool, gallery, "comment", 31).await;

        for path in [
            "/api/v1/timeline".to_owned(),
            format!("/api/v1/galleries/{}/timeline", gallery.1.0),
        ] {
            let (status, oldest) = call(&app, get(&format!("{path}?sort=oldest"), &cookie)).await;
            let (invalid, _) = call(&app, get(&format!("{path}?sort=invalid"), &cookie)).await;

            assert_eq!(status, StatusCode::OK);
            assert_eq!(oldest["events"][0]["kind"], "comment");
            assert_eq!(invalid, StatusCode::BAD_REQUEST);
        }

        let (_, first) = call(&app, get("/api/v1/timeline", &cookie)).await;

        assert_eq!(first["events"].as_array().unwrap().len(), 30);

        let cursor = first["next"].as_str().unwrap();
        let (_, second) = call(
            &app,
            get(&format!("/api/v1/timeline?cursor={cursor}"), &cookie),
        )
        .await;

        assert_eq!(second["events"].as_array().unwrap().len(), 1);
        assert_eq!(second["events"][0]["kind"], "comment");
        assert_eq!(second["next"], Value::Null);

        let (_, comments) = call(&app, get("/api/v1/timeline?kind=comment", &cookie)).await;

        assert_eq!(comments["events"].as_array().unwrap().len(), 1);

        let (_, anonymous) = call(
            &app,
            get("/api/v1/timeline?contact=anonymous&range=7d", &cookie),
        )
        .await;

        assert_eq!(anonymous["events"].as_array().unwrap().len(), 30);

        for uri in [
            "/api/v1/timeline?cursor=garbage",
            "/api/v1/timeline?contact=nobody",
            "/api/v1/timeline?kind=unknown",
            "/api/v1/timeline?range=1y",
        ] {
            let (status, _) = call(&app, get(uri, &cookie)).await;

            assert_eq!(status, StatusCode::BAD_REQUEST, "{uri}");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn another_workspaces_gallery_is_not_found_and_its_lines_stay_hidden(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let workspace_id = other_workspace(&pool).await;
        let foreign = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(workspace_id)
            .await
            .unwrap()
            .id;

        insert_line(&pool, (workspace_id, foreign), "open", 1).await;

        let (status, _) = call(
            &app,
            get(
                &format!("/api/v1/galleries/{}/timeline?q=anonyme", foreign.0),
                &cookie,
            ),
        )
        .await;
        let (_, feed) = call(&app, get("/api/v1/timeline?q=anonyme", &cookie)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(feed["events"].as_array().unwrap().is_empty());
    }
}
