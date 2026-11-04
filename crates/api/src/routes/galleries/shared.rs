//! Public client routes under `/api/v1/galleries/shared/{token}`.
//!
//! - `GET /` returns the title, layout and expiry of the gallery behind a private link.
//! - `GET /photos` lists the photos the client may see, with their displayed size.
//! - `GET /photos/{photo_id}/thumbnail|image` serves one of those photos.
//!
//! Every route takes [`ClientAccess`], so malformed, unknown, unshared and expired links are
//! the same 404, and so are unready, foreign and unknown photos. No route serves an original.
//! The link is a bearer credential: any access log added later must redact
//! `/galleries/shared/` paths.

use actix_web::{
    HttpResponse, get,
    web::{Data, Path, ServiceConfig},
};
use pp_core::notifications::PhotoId;
use pp_database::photo::PhotoRepository;
use pp_storage::{PhotoKey, Variant};
use serde::Deserialize;

use super::photos::read_image;
use crate::{
    api::ApiState,
    error::ErrorResponse,
    extractors::client_access::{ClientAccess, ClientPhotoSet},
    response::{ImageFile, ResponseBuilderExt},
};

/// Registers the client routes inside the `/api/v1/galleries` scope.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(show_shared_gallery)
        .service(list_shared_photos)
        .service(serve_shared_photo);
}

/// The image a client asks for; which file answers it depends on the photo set.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ClientImage {
    Thumbnail,
    Image,
}

impl ClientImage {
    fn variant(self, set: ClientPhotoSet) -> Variant {
        match (set, self) {
            (ClientPhotoSet::Proofs, Self::Thumbnail) => Variant::Thumbnail,
            (ClientPhotoSet::Proofs, Self::Image) => Variant::Preview,
        }
    }
}

#[get("/shared/{token}")]
async fn show_shared_gallery(
    ClientAccess(access): ClientAccess,
) -> Result<HttpResponse, ErrorResponse> {
    Ok(HttpResponse::Ok()
        .no_store()
        .no_referrer()
        .json(access.gallery))
}

#[get("/shared/{token}/photos")]
async fn list_shared_photos(
    state: Data<ApiState>,
    access: ClientAccess,
) -> Result<HttpResponse, ErrorResponse> {
    let photos = state.database().repository::<PhotoRepository>();
    let photos = match access.photo_set() {
        ClientPhotoSet::Proofs => photos.list_ready(access.0.location).await?,
    };

    Ok(HttpResponse::Ok().no_store().no_referrer().json(photos))
}

#[get("/shared/{token}/photos/{photo_id}/{image:thumbnail|image}")]
async fn serve_shared_photo(
    state: Data<ApiState>,
    access: ClientAccess,
    path: Path<(String, PhotoId, ClientImage)>,
) -> Result<ImageFile, ErrorResponse> {
    let (_, photo_id, image) = path.into_inner();
    let location = access.0.location;
    let set = access.photo_set();
    let visible = match set {
        ClientPhotoSet::Proofs => {
            state
                .database()
                .repository::<PhotoRepository>()
                .is_ready(location, photo_id)
                .await?
        }
    };

    if !visible {
        return Err(ErrorResponse::not_found());
    }

    read_image(
        &state,
        &PhotoKey::with_location(location, photo_id, image.variant(set)),
    )
    .await
}

#[cfg(test)]
mod tests {
    use actix_web::{
        App,
        cookie::Cookie,
        dev::{Service, ServiceResponse},
        http::StatusCode,
        test,
    };
    use pp_database::{
        Database,
        gallery::{GalleryChanges, GalleryRepository, ShareToken},
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use super::*;
    use crate::{
        api::configure_routes,
        routes::galleries::photos::tests::{
            Progress, app, call_request, create_gallery, photo_in, store_derivative, upload_config,
            waiting_photo,
        },
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

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
        test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}"))
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn shared_page_returns_only_client_fields(pool: PgPool) {
        let token = shared_gallery(&pool, json!({"title": "Wedding", "layout": "masonry"})).await;
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
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

    /// Shares the gallery through the photographer route and returns its token.
    async fn share<S>(app: &S, cookie: &Cookie<'static>, gallery_id: &str) -> String
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/share"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone());
        let (status, gallery) = call(app, request).await;

        assert_eq!(status, StatusCode::OK);

        gallery["share_token"].as_str().unwrap().to_owned()
    }

    async fn expire<S>(app: &S, cookie: &Cookie<'static>, gallery_id: &str)
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::patch()
            .uri(&format!("/api/v1/galleries/{gallery_id}"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .set_json(json!({"expires_at": "2000-01-01T00:00:00Z"}));

        assert_eq!(call(app, request).await.0, StatusCode::OK);
    }

    fn client_request(token: &str, path: &str) -> actix_http::Request {
        test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/shared/{token}{path}"))
            .to_request()
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn client_lists_only_ready_photos_with_dimensions(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let first = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        photo_in(&app, &pool, &cookie, &gallery_id, Progress::Processing).await;
        photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;

        let second = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        waiting_photo(&app, &cookie, &gallery_id).await;

        let token = share(&app, &cookie, &gallery_id).await;
        let response = test::call_service(&app, client_request(&token, "/photos")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );

        let body: Value = test::read_body_json(response).await;

        assert_eq!(
            body,
            json!([
                {"id": first, "width": 2, "height": 1},
                {"id": second, "width": 2, "height": 1},
            ])
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn client_serves_thumbnail_and_image_of_a_ready_photo(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let token = share(&app, &cookie, &gallery_id).await;

        for (image, variant) in [("thumbnail", "thumbnail"), ("image", "preview")] {
            let stored = store_derivative(&pool, directory.path(), &photo, variant).await;
            let response = test::call_service(
                &app,
                client_request(&token, &format!("/photos/{photo}/{image}")),
            )
            .await;
            let headers = response.headers();

            assert_eq!(response.status(), StatusCode::OK, "{image}");
            assert_eq!(headers.get("content-type").unwrap(), "image/jpeg");
            assert_eq!(
                headers.get("cache-control").unwrap(),
                "private, max-age=3600"
            );
            assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
            assert_eq!(headers.get("referrer-policy").unwrap(), "no-referrer");
            assert_eq!(test::read_body(response).await, stored);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn client_media_is_404_for_unready_foreign_unknown_and_missing_files(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let without_file = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let failed = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;
        let foreign = photo_in(&app, &pool, &cookie, &other_gallery, Progress::Ready).await;
        let waiting = waiting_photo(&app, &cookie, &gallery_id).await;

        for photo in [&failed, &foreign, &waiting] {
            store_derivative(&pool, directory.path(), photo, "thumbnail").await;
            store_derivative(&pool, directory.path(), photo, "preview").await;
        }

        let token = share(&app, &cookie, &gallery_id).await;
        let unknown = uuid::Uuid::new_v4().to_string();

        for photo in [&without_file, &failed, &foreign, &waiting, &unknown] {
            for image in ["thumbnail", "image"] {
                let (status, _) = call_request(
                    &app,
                    client_request(&token, &format!("/photos/{photo}/{image}")),
                )
                .await;

                assert_eq!(status, StatusCode::NOT_FOUND, "{photo} {image}");
            }
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn expired_and_unshared_links_deny_list_and_media(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        store_derivative(&pool, directory.path(), &photo, "thumbnail").await;
        store_derivative(&pool, directory.path(), &photo, "preview").await;

        let token = share(&app, &cookie, &gallery_id).await;
        let paths = [
            "/photos".to_owned(),
            format!("/photos/{photo}/thumbnail"),
            format!("/photos/{photo}/image"),
        ];

        for path in &paths {
            let (status, _) = call_request(&app, client_request(&token, path)).await;

            assert_eq!(status, StatusCode::OK, "{path}");
        }

        expire(&app, &cookie, &gallery_id).await;

        let unshared = ShareToken::generate().as_str().to_owned();

        for token in [token.as_str(), unshared.as_str(), "short"] {
            for path in &paths {
                let (status, _) = call_request(&app, client_request(token, path)).await;

                assert_eq!(status, StatusCode::NOT_FOUND, "{token}{path}");
            }
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn no_client_route_serves_the_original(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        store_derivative(&pool, directory.path(), &photo, "preview").await;

        let token = share(&app, &cookie, &gallery_id).await;

        for path in ["original", "Original", "preview", "download", ""] {
            let (status, _) = call_request(
                &app,
                client_request(&token, &format!("/photos/{photo}/{path}")),
            )
            .await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        }
    }
}
