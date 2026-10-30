//! Gallery endpoints under `/api/v1/galleries`, scoped to the session's workspace.
//!
//! - `POST /` creates an untitled gallery with the default settings.
//! - `GET /?sort=updated|created|title` lists the workspace's galleries.
//! - `GET /{id}` returns one gallery.
//! - `PATCH /{id}` applies a [`GalleryChanges`] changeset and returns the updated gallery.
//! - `POST /{id}/share` creates the gallery's private link if it has none and returns the
//!   gallery; sharing again returns the same link.
//!
//! - `GET /api/v1/shared/{token}` is public: it returns the title, layout and expiry of the
//!   gallery behind a private link. Malformed, unknown and expired links are the same 404 (a
//!   malformed token fails to deserialize into [`ShareToken`], so it never reaches the
//!   database). The link is a bearer credential: any access log added later must redact
//!   `/shared/` paths.
//!
//! A gallery of another workspace is a 404, never a 403, so ids reveal nothing. Like every state-changing
//! route, they must carry the configured `Origin`, which the API-wide middleware checks.

use actix_web::{
    HttpResponse, get, patch, post,
    web::{self, Data, Json, Path, Query, ServiceConfig},
};
use pp_database::gallery::{GalleryChanges, GalleryId, GallerySort, ShareToken};
use serde::Deserialize;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    sort: GallerySort,
}

/// Registers the gallery routes.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(fetch_shared_gallery).service(
        web::scope("/galleries")
            .service(create_gallery)
            .service(list_galleries)
            .service(show_gallery)
            .service(update_gallery)
            .service(share_gallery),
    );
}

#[post("")]
async fn create_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state.galleries().create(identity.workspace_id).await?;

    Ok(HttpResponse::Created().no_store().json(gallery))
}

#[get("")]
async fn list_galleries(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    query: Query<ListQuery>,
) -> Result<HttpResponse, ErrorResponse> {
    let galleries = state
        .galleries()
        .list(identity.workspace_id, query.sort)
        .await?;

    Ok(HttpResponse::Ok().no_store().json(galleries))
}

#[get("/{id}")]
async fn show_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .galleries()
        .get(identity.workspace_id, id.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(gallery))
}

#[patch("/{id}")]
async fn update_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
    changes: Json<GalleryChanges>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .galleries()
        .update(identity.workspace_id, id.into_inner(), &changes)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(gallery))
}

#[post("/{id}/share")]
async fn share_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .galleries()
        .share(identity.workspace_id, id.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(gallery))
}

#[get("/shared/{token}")]
async fn fetch_shared_gallery(
    state: Data<ApiState>,
    token: Path<ShareToken>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .galleries()
        .find_shared(&token.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().no_referrer().json(gallery))
}

#[cfg(test)]
mod tests {
    use actix_web::{App, cookie::Cookie, http::StatusCode, test};
    use pp_database::{
        Database,
        gallery::{GalleryChanges, GalleryRepository},
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use super::*;
    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

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
                .app_data(Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        let (status, created) = call(&app, post_gallery(&cookie)).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["title"], Value::Null);
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

        let (_, untitled) = call(&app, patch(&id, &cookie, json!({"title": "  "}))).await;

        assert_eq!(untitled["title"], Value::Null);

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
    async fn responses_are_never_cached(pool: PgPool) {
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let created = test::call_service(&app, post_gallery(&cookie).to_request()).await;

        assert_eq!(created.status(), StatusCode::CREATED);
        assert_eq!(created.headers().get("Cache-Control").unwrap(), "no-store");

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries")
            .cookie(cookie);
        let listed = test::call_service(&app, request.to_request()).await;

        assert_eq!(listed.headers().get("Cache-Control").unwrap(), "no-store");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn invalid_changes_and_requests_are_rejected(pool: PgPool) {
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, created) = call(&app, post_gallery(&cookie)).await;
        let id = created["id"].as_str().unwrap().to_owned();
        let long = "x".repeat(256);

        for (body, field) in [
            (json!({"title": long}), Some("title")),
            (json!({"included_photos": 0}), None),
            (
                json!({"included_photos": u64::MAX}),
                Some("included_photos"),
            ),
            (json!({"layout": null}), None),
            (json!({"watermark_enabled": null}), None),
            (json!({"layout": "carousel"}), None),
            (json!({"unknown": 1}), None),
            (json!({"expires_at": "tomorrow"}), None),
            (json!({"expires_at": "9999-12-31T23:59:59-01:00"}), None),
        ] {
            let (status, error) = call(&app, patch(&id, &cookie, body.clone())).await;

            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
            assert_eq!(
                error["context"].get(0).and_then(Value::as_str),
                field,
                "{body}"
            );
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

        assert_eq!(after["title"], Value::Null);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn session_and_origin_are_required(pool: PgPool) {
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
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
                .app_data(Data::new(state))
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

    fn share(
        id: &str,
        cookie: Option<&Cookie<'static>>,
        origin: Option<&str>,
    ) -> test::TestRequest {
        let mut request = test::TestRequest::post().uri(&format!("/api/v1/galleries/{id}/share"));

        if let Some(cookie) = cookie {
            request = request.cookie(cookie.clone());
        }

        if let Some(origin) = origin {
            request = request.insert_header(("Origin", origin));
        }

        request
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn sharing_returns_the_same_link_and_requires_session_and_origin(pool: PgPool) {
        let foreign = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap();
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, created) = call(&app, post_gallery(&cookie)).await;
        let id = created["id"].as_str().unwrap();

        assert_eq!(created["share_token"], Value::Null);

        let (status, first) = call(&app, share(id, Some(&cookie), Some(ORIGIN))).await;
        let (_, second) = call(&app, share(id, Some(&cookie), Some(ORIGIN))).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(first["share_token"].as_str().map(str::len), Some(22));
        assert_eq!(first["share_token"], second["share_token"]);

        let (status, _) = call(&app, share(id, None, Some(ORIGIN))).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, _) = call(&app, share(id, Some(&cookie), None)).await;

        assert_eq!(status, StatusCode::FORBIDDEN);

        let foreign_id = foreign.id.0.to_string();
        let (status, _) = call(&app, share(&foreign_id, Some(&cookie), Some(ORIGIN))).await;

        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(&app, share("not-a-uuid", Some(&cookie), Some(ORIGIN))).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    async fn shared_gallery(pool: &PgPool, changes: Value) -> String {
        let workspace = other_workspace(pool).await;

        let database = Database::new(pool.clone());
        let store = database.repository::<GalleryRepository>();
        let id = store.create(workspace).await.unwrap().id;
        let changes: GalleryChanges = serde_json::from_value(changes).unwrap();

        store.update(workspace, id, &changes).await.unwrap();

        store
            .share(workspace, id)
            .await
            .unwrap()
            .unwrap()
            .share_token
            .unwrap()
            .as_str()
            .to_owned()
    }

    fn get(token: &str) -> test::TestRequest {
        test::TestRequest::get().uri(&format!("/api/v1/shared/{token}"))
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shared_page_returns_only_client_fields(pool: PgPool) {
        let token = shared_gallery(&pool, json!({"title": "Wedding", "layout": "masonry"})).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;

        let response = test::call_service(&app, get(&token).to_request()).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );

        let body: Value = test::read_body_json(response).await;

        assert_eq!(
            body,
            json!({"title": "Wedding", "layout": "masonry", "expires_at": null})
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn unknown_malformed_and_expired_links_are_indistinguishable(pool: PgPool) {
        let expired = shared_gallery(&pool, json!({"expires_at": "2000-01-01T00:00:00Z"})).await;
        let live = shared_gallery(&pool, json!({})).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let swapped_case: String = live
            .chars()
            .map(|c| {
                if c.is_ascii_lowercase() {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
        let unknown = "A".repeat(22);
        let non_canonical = format!("{}B", "A".repeat(21));
        let mut outcomes = Vec::new();

        for token in [
            expired.as_str(),
            unknown.as_str(),
            non_canonical.as_str(),
            "short",
            swapped_case.as_str(),
        ] {
            let response = test::call_service(&app, get(token).to_request()).await;
            let status = response.status();
            let mut headers: Vec<_> = response
                .headers()
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_str().unwrap().to_owned()))
                .collect();

            headers.sort();

            outcomes.push((status, headers, test::read_body(response).await));
        }

        assert_eq!(outcomes[0].0, StatusCode::NOT_FOUND);
        assert!(outcomes.windows(2).all(|pair| pair[0] == pair[1]));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn extending_the_expiry_restores_access_at_the_same_link(pool: PgPool) {
        let app = test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, created) = call(
            &app,
            test::TestRequest::post()
                .uri("/api/v1/galleries")
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone()),
        )
        .await;
        let id = created["id"].as_str().unwrap();
        let patch = |value: Value| {
            test::TestRequest::patch()
                .uri(&format!("/api/v1/galleries/{id}"))
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone())
                .set_json(json!({"expires_at": value}))
        };
        let (_, shared) = call(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/{id}/share"))
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone()),
        )
        .await;
        let token = shared["share_token"].as_str().unwrap();

        call(&app, patch(json!("2000-01-01T00:00:00Z"))).await;

        assert_eq!(call(&app, get(token)).await.0, StatusCode::NOT_FOUND);

        call(&app, patch(json!("2999-01-01T00:00:00Z"))).await;

        let (status, body) = call(&app, get(token)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["expires_at"], "2999-01-01T00:00:00Z");
    }
}
