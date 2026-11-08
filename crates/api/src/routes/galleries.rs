//! Gallery endpoints under `/api/v1/galleries`, scoped to the session's workspace.
//!
//! - `POST /` creates an untitled gallery with the default settings.
//! - `GET /?sort=updated|created|oldest|title&state=…` lists the workspace's galleries. `state` is
//!   `draft|shared|delivered|archived|expired`, may repeat, and keeps galleries in any of the
//!   given states; unknown or empty values are a 400.
//! - `GET /{id}` returns one gallery.
//! - `PATCH /{id}` applies a [`GalleryChanges`] changeset and returns the updated gallery.
//! - `POST /{id}/share` creates the gallery's private link if it has none and returns the
//!   gallery; sharing again returns the same link.
//! - `POST /{id}/deliver` switches the link to the finished photos and returns the gallery;
//!   delivering again changes nothing, and a gallery not shared or archived is a 409.
//! - `POST /{id}/archive` hides the gallery from its client and returns it; archiving again
//!   changes nothing.
//! - `POST /{id}/restore` makes an archived gallery reachable again and returns it.
//! - `DELETE /{id}` deletes the gallery and its photos at once (204); the worker removes the
//!   files afterwards.
//!
//! The public `/shared/{token}` routes for clients live in [`shared`].
//!
//! A gallery of another workspace is a 404, never a 403, so ids reveal nothing. Like every state-changing
//! route, they must carry the configured `Origin`, which the API-wide middleware checks.

use actix_web::{
    HttpRequest, HttpResponse, delete, get, patch, post,
    web::{self, Data, Json, Path, ServiceConfig},
};
use actix_web_lab::extract::Query as LabQuery;
use pp_database::gallery::{
    Delivery, GalleryChanges, GalleryId, GalleryRepository, GallerySort, GalleryState,
};
use pp_database::photo::{PhotoLocation, Watermarked};
use pp_database::settings::SettingsRepository;
use pp_database::workspace_deletion::WorkspaceDeletion;
use pp_storage::Variant;
use serde::Deserialize;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

mod export;
mod photos;
mod shared;

/// The photo files photographer and client routes may serve; the original is deliberately absent.
/// Each route pattern lists the same names so any other path misses it, even before
/// authentication.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DerivativePhoto {
    Thumbnail,
    Preview,
}

impl DerivativePhoto {
    fn variant(self, watermarked: Watermarked) -> Variant {
        match (self, watermarked.0) {
            (Self::Thumbnail, false) => Variant::Thumbnail,
            (Self::Preview, false) => Variant::Preview,
            (Self::Thumbnail, true) => Variant::WatermarkedThumbnail,
            (Self::Preview, true) => Variant::WatermarkedPreview,
        }
    }
}

/// The list query. `state` may repeat, which actix's own `Query` cannot read, so this goes
/// through the HTML-form decoder of `actix-web-lab`.
#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    sort: GallerySort,
    #[serde(default, rename = "state")]
    states: Vec<GalleryState>,
}

/// Registers the gallery routes.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/galleries")
            .service(create_gallery)
            .service(list_galleries)
            .configure(shared::configure)
            .service(show_gallery)
            .service(update_gallery)
            .service(share_gallery)
            .service(deliver_gallery)
            .service(archive_gallery)
            .service(restore_gallery)
            .service(delete_gallery)
            .configure(photos::configure)
            .configure(export::configure),
    );
}

#[post("")]
async fn create_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .database()
        .repository::<GalleryRepository>()
        .create(identity.workspace_id)
        .await?;

    Ok(HttpResponse::Created().no_store().json(gallery))
}

#[get("")]
async fn list_galleries(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    request: HttpRequest,
) -> Result<HttpResponse, ErrorResponse> {
    let ListQuery { sort, states } = LabQuery::<ListQuery>::from_query(request.query_string())
        .map_err(|_| ErrorResponse::bad_request())?
        .into_inner();

    let galleries = state
        .database()
        .repository::<GalleryRepository>()
        .list(identity.workspace_id, sort, &states)
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
        .database()
        .repository::<GalleryRepository>()
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
        .database()
        .repository_with::<SettingsRepository>(state.job_options())
        .update_gallery(identity.workspace_id, id.into_inner(), &changes)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(gallery))
}

#[post("/{id}/archive")]
async fn archive_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .database()
        .repository::<GalleryRepository>()
        .archive(identity.workspace_id, id.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(gallery))
}

#[post("/{id}/restore")]
async fn restore_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .database()
        .repository::<GalleryRepository>()
        .restore(identity.workspace_id, id.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(gallery))
}

#[delete("/{id}")]
async fn delete_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let found = state
        .database()
        .repository_with::<WorkspaceDeletion>(state.job_options())
        .delete_gallery(identity.workspace_id, id.into_inner())
        .await?;

    if !found {
        return Err(ErrorResponse::not_found());
    }

    Ok(HttpResponse::NoContent().no_store().finish())
}

#[post("/{id}/share")]
async fn share_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery = state
        .database()
        .repository::<GalleryRepository>()
        .share(identity.workspace_id, id.into_inner())
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(gallery))
}

#[post("/{id}/deliver")]
async fn deliver_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: Path<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    match state
        .database()
        .repository::<GalleryRepository>()
        .deliver(identity.workspace_id, id.into_inner())
        .await?
    {
        Delivery::Delivered(gallery) => {
            // A head start for the first click; the click builds it if this did not.
            let location = PhotoLocation {
                workspace_id: identity.workspace_id,
                gallery_id: gallery.id,
            };

            if let Err(error) = state.ensure_delivery(location).await {
                tracing::warn!(%error, "delivery ZIP could not be started");
            }

            Ok(HttpResponse::Ok().no_store().json(gallery))
        }
        Delivery::NotDeliverable => Err(ErrorResponse::conflict("gallery is not shared")),
        Delivery::NotFound => Err(ErrorResponse::not_found()),
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{App, cookie::Cookie, http::StatusCode, test};
    use pp_database::{Database, gallery::GalleryRepository};
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use uuid::Uuid;

    use super::*;
    use crate::{
        api::configure_routes,
        routes::galleries::photos::tests::{Progress, create_gallery, photo_in},
        tests::{ORIGIN, app, app_state, call, other_workspace, sign_up, upload_config},
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
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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
    async fn listing_reports_photo_and_favorite_counts_and_sorts_oldest_first(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let first = create_gallery(&app, &cookie).await;
        let second = create_gallery(&app, &cookie).await;
        let chosen = photo_in(&app, &pool, &cookie, &first, Progress::Ready).await;

        photo_in(&app, &pool, &cookie, &first, Progress::Ready).await;
        photo_in(&app, &pool, &cookie, &first, Progress::Processing).await;

        sqlx::query("UPDATE photos SET favorite = true WHERE id = $1::uuid")
            .bind(&chosen)
            .execute(&pool)
            .await
            .unwrap();

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries?sort=oldest")
            .cookie(cookie);
        let (status, list) = call(&app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(list[0]["id"], first.as_str());
        assert_eq!(
            (&list[0]["photo_count"], &list[0]["favorite_count"]),
            (&json!(2), &json!(1))
        );
        assert_eq!(list[1]["id"], second.as_str());
        assert_eq!(
            (&list[1]["photo_count"], &list[1]["favorite_count"]),
            (&json!(0), &json!(0))
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_filters_by_state_and_stays_in_the_workspace(pool: PgPool) {
        let foreign = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap();
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, draft) = call(&app, post_gallery(&cookie)).await;
        let (_, expired) = call(&app, post_gallery(&cookie)).await;
        let (_, shared) = call(&app, post_gallery(&cookie)).await;
        let shared_id = shared["id"].as_str().unwrap();
        let expired_id = expired["id"].as_str().unwrap();

        call(
            &app,
            test::TestRequest::post()
                .uri(&format!("/api/v1/galleries/{shared_id}/share"))
                .insert_header(("Origin", ORIGIN))
                .cookie(cookie.clone()),
        )
        .await;
        call(
            &app,
            patch(
                expired_id,
                &cookie,
                json!({"expires_at": "2000-01-01T00:00:00Z"}),
            ),
        )
        .await;

        let ids = |uri: &'static str| {
            let (app, cookie) = (&app, cookie.clone());

            async move {
                let request = test::TestRequest::get().uri(uri).cookie(cookie);
                let (status, body) = call(app, request).await;

                (
                    status,
                    body.as_array()
                        .map(|list| {
                            list.iter()
                                .map(|gallery| gallery["id"].as_str().unwrap().to_owned())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default(),
                )
            }
        };

        let (status, listed) =
            ids("/api/v1/galleries?state=shared&state=expired&sort=created").await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed, [shared_id, expired_id]);

        let (_, listed) = ids("/api/v1/galleries?state=shared&state=shared").await;

        assert_eq!(listed, [shared_id]);

        let (_, listed) = ids("/api/v1/galleries?state=draft&sort=title").await;

        let mut expected = [draft["id"].as_str().unwrap(), expired_id];

        expected.sort_unstable();

        assert_eq!(listed, expected);
        assert!(!listed.contains(&foreign.id.0.to_string()));

        let (_, all) = ids("/api/v1/galleries").await;

        assert_eq!(all.len(), 3);

        let request = test::TestRequest::get()
            .uri("/api/v1/galleries?state=expired")
            .cookie(cookie.clone());
        let (_, body) = call(&app, request).await;

        assert_eq!(body[0]["states"], json!(["draft", "expired"]));

        for uri in [
            "/api/v1/galleries?state=bogus",
            "/api/v1/galleries?state=",
            "/api/v1/galleries?state=shared&sort=bogus",
        ] {
            assert_eq!(ids(uri).await.0, StatusCode::BAD_REQUEST, "{uri}");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn hero_photo_round_trips_and_rejects_unusable_photos(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery_id = create_gallery(&app, &cookie).await;
        let first = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let second = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let processing = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Processing).await;
        let foreign = photo_in(&app, &pool, &cookie, &other_gallery_id, Progress::Ready).await;

        for expected in [Value::from(first.clone()), Value::from(second.clone())] {
            let (status, body) = call(
                &app,
                patch(&gallery_id, &cookie, json!({"hero_id": expected})),
            )
            .await;

            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["hero_id"], expected);
        }

        for rejected in [processing, foreign, Uuid::new_v4().to_string()] {
            let (status, _) = call(
                &app,
                patch(&gallery_id, &cookie, json!({"hero_id": rejected})),
            )
            .await;

            assert_eq!(status, StatusCode::NOT_FOUND);
        }

        let (_, kept) = call(&app, patch(&gallery_id, &cookie, json!({"title": "T"}))).await;

        assert_eq!(kept["hero_id"], second.as_str());

        let (status, cleared) =
            call(&app, patch(&gallery_id, &cookie, json!({"hero_id": null}))).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(cleared["hero_id"], Value::Null);

        let (status, _) = call(
            &app,
            patch(&gallery_id, &cookie, json!({"hero_id": "nope"})),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn responses_are_never_cached(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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
            .uri("/api/v1/galleries?sort=bogus")
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
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        let foreign = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap()
            .id
            .0
            .to_string();
        let foreign = foreign.as_str();
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
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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
}
