//! Gallery endpoints under `/api/v1/galleries`, scoped to the session's workspace.
//!
//! - `POST /` creates a gallery with the default title and settings.
//! - `GET /?sort=updated|created|title` lists the workspace's galleries.
//! - `GET /{id}` returns one gallery.
//! - `PATCH /{id}` applies a [`GalleryChanges`] changeset and returns the updated gallery.
//!
//! A gallery of another workspace is a 404, never a 403, so ids reveal nothing. Like the auth
//! routes, state-changing requests must carry the configured `Origin`.

use actix_web::{
    HttpRequest, HttpResponse, error::JsonPayloadError, get, middleware::from_fn, patch, post, web,
};
use pp_database::gallery::{GalleryChanges, GalleryId, GallerySort};
use serde::{Deserialize, Serialize};

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    origin::require_same_origin,
};

/// A changeset is a handful of settings; this bounds the body well above any valid one.
const CHANGES_LIMIT: usize = 4096;

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    sort: GallerySort,
}

/// Registers the gallery routes.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/galleries")
            .wrap(from_fn(require_same_origin))
            .app_data(
                web::JsonConfig::default()
                    .limit(CHANGES_LIMIT)
                    .error_handler(|error: JsonPayloadError, _: &HttpRequest| {
                        ErrorResponse::from(error).into()
                    }),
            )
            .app_data(
                web::QueryConfig::default()
                    .error_handler(|_, _: &HttpRequest| ErrorResponse::bad_request().into()),
            )
            .app_data(
                web::PathConfig::default()
                    .error_handler(|_, _: &HttpRequest| ErrorResponse::not_found().into()),
            )
            .service(create_gallery)
            .service(list_galleries)
            .service(show_gallery)
            .service(update_gallery),
    );
}

fn no_store<T: Serialize>(mut response: actix_web::HttpResponseBuilder, body: &T) -> HttpResponse {
    response
        .insert_header((actix_web::http::header::CACHE_CONTROL, "no-store"))
        .json(body)
}

#[post("")]
async fn create_gallery(
    state: web::Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state.galleries().create(identity.workspace_id).await?;

    Ok(no_store(HttpResponse::Created(), &gallery))
}

#[get("")]
async fn list_galleries(
    state: web::Data<ApiState>,
    Authenticated(identity): Authenticated,
    query: web::Query<ListQuery>,
) -> Result<HttpResponse, ErrorResponse> {
    let galleries = state
        .galleries()
        .list(identity.workspace_id, query.sort)
        .await?;

    Ok(no_store(HttpResponse::Ok(), &galleries))
}

#[get("/{id}")]
async fn show_gallery(
    state: web::Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: web::Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .galleries()
        .get(identity.workspace_id, id.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(no_store(HttpResponse::Ok(), &gallery))
}

#[patch("/{id}")]
async fn update_gallery(
    state: web::Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: web::Path<GalleryId>,
    changes: web::Json<GalleryChanges>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .galleries()
        .update(identity.workspace_id, id.into_inner(), &changes)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(no_store(HttpResponse::Ok(), &gallery))
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroUsize};

    use actix_web::{
        App,
        cookie::Cookie,
        dev::{Service, ServiceResponse},
        http::StatusCode,
        test,
    };
    use pp_database::Database;
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use super::*;
    use crate::{
        api::configure_routes,
        config::{AuthConfig, LimiterConfig, SessionConfig},
    };

    const ORIGIN: &str = "http://localhost:5173";

    fn app_state(pool: PgPool) -> ApiState {
        let session = SessionConfig {
            idle_timeout: "1h".parse().unwrap(),
            cookie_name: "pp_session".parse().unwrap(),
        };
        let auth = AuthConfig {
            app_origin: ORIGIN.parse().unwrap(),
        };
        let limiter = LimiterConfig {
            max_failures: NonZeroU32::new(10).unwrap(),
            window: "1m".parse().unwrap(),
            max_peers: NonZeroUsize::new(1024).unwrap(),
        };

        ApiState::new(Database::new(pool), session, auth, limiter).unwrap()
    }

    async fn call<S>(app: &S, request: test::TestRequest) -> (StatusCode, Value)
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let response = test::call_service(app, request.to_request()).await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

        (status, body)
    }

    async fn sign_up<S>(app: &S) -> Cookie<'static>
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::post()
            .uri("/api/v1/auth/setup")
            .insert_header(("Origin", ORIGIN))
            .set_json(json!({"email": "person@example.test", "password": "a long test password"}))
            .to_request();
        let response = test::call_service(app, request).await;

        response
            .response()
            .cookies()
            .find(|cookie| cookie.name() == "pp_session")
            .unwrap()
            .into_owned()
    }

    fn patch(id: &str, cookie: &Cookie<'static>, body: Value) -> test::TestRequest {
        test::TestRequest::patch()
            .uri(&format!("/api/v1/galleries/{id}"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .set_json(body)
    }

    fn post_gallery(cookie: &Cookie<'static>) -> test::TestRequest {
        test::TestRequest::post()
            .uri("/api/v1/galleries")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn creating_listing_and_patching_round_trip(pool: PgPool) {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        let (status, created) = call(&app, post_gallery(&cookie)).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["title"], "Untitled gallery");
        assert_eq!(created["layout"], "grid");
        assert_eq!(created["expires_at"], Value::Null);
        assert_eq!(created["included_photos"], Value::Null);
        assert_eq!(created["watermark_enabled"], false);

        let id = created["id"].as_str().unwrap().to_owned();
        let (status, updated) = call(
            &app,
            patch(
                &id,
                &cookie,
                json!({
                    "title": " Wedding ",
                    "layout": "masonry",
                    "expires_at": "2030-01-02T03:04:05Z",
                    "included_photos": 30,
                    "watermark_enabled": true,
                }),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["title"], "Wedding");
        assert_eq!(updated["layout"], "masonry");
        assert_eq!(updated["expires_at"], "2030-01-02T03:04:05Z");
        assert_eq!(updated["included_photos"], 30);

        let (_, cleared) = call(&app, patch(&id, &cookie, json!({"expires_at": null}))).await;

        assert_eq!(cleared["expires_at"], Value::Null);
        assert_eq!(cleared["included_photos"], 30);
        assert_eq!(cleared["title"], "Wedding");

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries/")
            .cookie(cookie.clone());
        let (status, _) = call(&app, request).await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries?sort=title")
            .cookie(cookie.clone());
        let (status, list) = call(&app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(list.as_array().unwrap().len(), 1);

        let request = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{id}"))
            .cookie(cookie);
        let (status, shown) = call(&app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(shown["id"], id.as_str());
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn invalid_changes_and_requests_are_rejected(pool: PgPool) {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, created) = call(&app, post_gallery(&cookie)).await;
        let id = created["id"].as_str().unwrap().to_owned();
        let long = "x".repeat(256);

        for (body, field) in [
            (json!({"title": "   "}), Some("title")),
            (json!({"title": long}), Some("title")),
            (json!({"included_photos": 0}), None),
            (
                json!({"included_photos": u64::MAX}),
                Some("included_photos"),
            ),
            (json!({"title": null}), None),
            (json!({"layout": null}), None),
            (json!({"watermark_enabled": null}), None),
            (json!({"layout": "carousel"}), None),
            (json!({"unknown": 1}), None),
            (json!({"expires_at": "tomorrow"}), None),
            (json!({"expires_at": "9999-12-31T23:59:59-01:00"}), None),
        ] {
            let (status, error) = call(&app, patch(&id, &cookie, body.clone())).await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
            assert_eq!(error["field"].as_str(), field, "{body}");
        }

        let (status, _) = call(
            &app,
            test::TestRequest::patch()
                .uri(&format!("/api/v1/galleries/{id}"))
                .insert_header(("Origin", ORIGIN))
                .insert_header(("Content-Type", "application/json"))
                .cookie(cookie.clone())
                .set_payload("{not json"),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries?sort=oldest")
            .cookie(cookie.clone());

        assert_eq!(call(&app, request).await.0, StatusCode::BAD_REQUEST);

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries/not-a-uuid")
            .cookie(cookie.clone());

        assert_eq!(call(&app, request).await.0, StatusCode::NOT_FOUND);

        let missing = "00000000-0000-4000-8000-000000000000";

        assert_eq!(
            call(
                &app,
                patch(missing, &cookie, json!({"watermark_enabled": true}))
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );

        let (_, after) = call(
            &app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/{id}"))
                .cookie(cookie),
        )
        .await;

        assert_eq!(after["title"], "Untitled gallery");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn session_and_origin_are_required(pool: PgPool) {
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, created) = call(&app, post_gallery(&cookie)).await;
        let id = created["id"].as_str().unwrap();

        let request = test::TestRequest::post()
            .uri("/api/v1/galleries")
            .insert_header(("Origin", ORIGIN));

        assert_eq!(call(&app, request).await.0, StatusCode::UNAUTHORIZED);

        let request = test::TestRequest::get().uri("/api/v1/galleries");

        assert_eq!(call(&app, request).await.0, StatusCode::UNAUTHORIZED);

        let request = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{id}"))
            .cookie(Cookie::new("pp_session", "x".repeat(43)));

        assert_eq!(call(&app, request).await.0, StatusCode::UNAUTHORIZED);

        let request = test::TestRequest::post()
            .uri("/api/v1/galleries")
            .cookie(cookie.clone());

        assert_eq!(call(&app, request).await.0, StatusCode::FORBIDDEN);

        let request = test::TestRequest::patch()
            .uri(&format!("/api/v1/galleries/{id}"))
            .insert_header(("Origin", "http://evil.example"))
            .cookie(cookie)
            .set_json(json!({"title": "Hijacked"}));

        assert_eq!(call(&app, request).await.0, StatusCode::FORBIDDEN);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn galleries_of_other_workspaces_are_not_found(pool: PgPool) {
        let state = app_state(pool.clone());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        sqlx::query("INSERT INTO workspaces(id) VALUES ('00000000-0000-4000-8000-0000000000aa')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO galleries(id,workspace_id,title) VALUES ('00000000-0000-4000-8000-0000000000bb','00000000-0000-4000-8000-0000000000aa','Private')")
            .execute(&pool)
            .await
            .unwrap();

        let foreign = "00000000-0000-4000-8000-0000000000bb";
        let request = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{foreign}"))
            .cookie(cookie.clone());

        assert_eq!(call(&app, request).await.0, StatusCode::NOT_FOUND);
        assert_eq!(
            call(&app, patch(foreign, &cookie, json!({"title": "Mine now"})))
                .await
                .0,
            StatusCode::NOT_FOUND
        );

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries")
            .cookie(cookie);
        let (_, list) = call(&app, request).await;

        assert!(list.as_array().unwrap().is_empty());
    }
}
