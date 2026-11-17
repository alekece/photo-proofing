//! The workspace activity log, under `/api/v1`.
//!
//! - `GET /activity` lists what clients did across the workspace's galleries, newest first,
//!   leaving out archived galleries.
//! - `GET /galleries/{id}/activity` lists one gallery's lines, archived or not; a gallery of
//!   another workspace is a 404.
//!
//! Both take the optional filters `contact` (a contact id or `anonymous`), `kind`, `range`
//! (`today`, `7d` or `30d`) and `cursor`, which is the `next` of the previous page. The workspace
//! feed also takes `gallery`. The log is the photographer's: it needs a session and names
//! contacts, which no client route does. A change arrives on the event stream as a
//! `notification` event.

use actix_web::{
    HttpResponse, get,
    web::{Data, Path, Query, ServiceConfig},
};
use pp_database::{
    activity::{
        ActivityCursor, ActivityKind, ActivityQuery, ActivityRange, ActivityRepository,
        ContactFilter,
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
    cfg.service(list_activity).service(list_gallery_activity);
}

#[derive(Deserialize)]
struct ActivityFilters {
    gallery: Option<GalleryId>,
    contact: Option<ContactFilter>,
    kind: Option<ActivityKind>,
    range: Option<ActivityRange>,
    cursor: Option<ActivityCursor>,
}

impl ActivityFilters {
    fn into_query(self, gallery: Option<GalleryId>, include_archived: bool) -> ActivityQuery {
        ActivityQuery {
            gallery,
            contact: self.contact,
            kind: self.kind,
            range: self.range,
            cursor: self.cursor,
            include_archived,
        }
    }
}

#[get("/activity")]
async fn list_activity(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    filters: Query<ActivityFilters>,
) -> Result<HttpResponse, ErrorResponse> {
    let filters = filters.into_inner();
    let gallery = filters.gallery;
    let page = state
        .database()
        .repository::<ActivityRepository>()
        .list(identity.workspace_id, filters.into_query(gallery, false))
        .await?;

    Ok(HttpResponse::Ok().no_store().json(page))
}

#[get("/galleries/{id}/activity")]
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
        .list(
            identity.workspace_id,
            filters.into_inner().into_query(Some(gallery_id), true),
        )
        .await?;

    Ok(HttpResponse::Ok().no_store().json(page))
}

#[cfg(test)]
mod tests {
    use actix_web::{App, http::StatusCode, test, web::Data};
    use pp_database::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
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

        let (status, _) = call(&app, test::TestRequest::get().uri("/api/v1/activity")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, feed) = call(&app, get("/api/v1/activity", &cookie)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(feed["events"].as_array().unwrap().len(), 1);
        assert_eq!(feed["events"][0]["kind"], "open");
        assert_eq!(feed["events"][0]["contact"], Value::Null);
        assert_eq!(feed["next"], Value::Null);

        let (status, own) = call(
            &app,
            get(
                &format!("/api/v1/galleries/{}/activity", archived.1.0),
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

        let (_, first) = call(&app, get("/api/v1/activity", &cookie)).await;

        assert_eq!(first["events"].as_array().unwrap().len(), 30);

        let cursor = first["next"].as_str().unwrap();
        let (_, second) = call(
            &app,
            get(&format!("/api/v1/activity?cursor={cursor}"), &cookie),
        )
        .await;

        assert_eq!(second["events"].as_array().unwrap().len(), 1);
        assert_eq!(second["events"][0]["kind"], "comment");
        assert_eq!(second["next"], Value::Null);

        let (_, comments) = call(&app, get("/api/v1/activity?kind=comment", &cookie)).await;

        assert_eq!(comments["events"].as_array().unwrap().len(), 1);

        let (_, anonymous) = call(
            &app,
            get("/api/v1/activity?contact=anonymous&range=7d", &cookie),
        )
        .await;

        assert_eq!(anonymous["events"].as_array().unwrap().len(), 30);

        for uri in [
            "/api/v1/activity?cursor=garbage",
            "/api/v1/activity?contact=nobody",
            "/api/v1/activity?kind=unknown",
            "/api/v1/activity?range=1y",
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
                &format!("/api/v1/galleries/{}/activity", foreign.0),
                &cookie,
            ),
        )
        .await;
        let (_, feed) = call(&app, get("/api/v1/activity", &cookie)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(feed["events"].as_array().unwrap().is_empty());
    }
}
