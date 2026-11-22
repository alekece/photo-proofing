//! Workspace-wide photo index under `/api/v1/photos`, feeding the All Photos page.
//!
//! `GET /` lists every ready proof and final from every gallery in the session's workspace,
//! regardless of gallery state, newest-first.

use actix_web::{
    HttpResponse, get,
    web::{self, Data, ServiceConfig},
};
use pp_database::photo::PhotoRepository;

use super::galleries;
use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

/// Registers the workspace photo index route.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/photos")
            .service(list_photos)
            .configure(galleries::export::workspace::configure),
    );
}

#[get("")]
async fn list_photos(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let photos = state
        .database()
        .repository::<PhotoRepository>()
        .list_ready_for_workspace(identity.workspace_id)
        .await?;

    Ok(HttpResponse::Ok().no_store().json(photos))
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU32, NonZeroU64};

    use actix_web::{App, cookie::Cookie, http::StatusCode, test, web::Data};
    use pp_database::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
        photo::{NewPhoto, PhotoDimensions, PhotoId, PhotoKind, PhotoLocation, PhotoStatus},
        photo_intake::PhotoIntake,
        tests::TestRepository,
    };
    use serde_json::Value;
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use uuid::Uuid;

    use crate::{
        api::configure_routes,
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

    fn post_gallery(cookie: &Cookie<'static>) -> test::TestRequest {
        test::TestRequest::post()
            .uri("/api/v1/galleries")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
    }

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

    /// Inserts a photo of `kind` directly (skipping upload/processing) and moves it to `status`
    /// with `created_at`, so ordering and inclusion/exclusion can be asserted without a worker.
    async fn photo_in(
        database: &Database,
        location: PhotoLocation,
        kind: PhotoKind,
        status: PhotoStatus,
        created_at: i64,
    ) -> PhotoId {
        let id = PhotoId::from(Uuid::new_v4());
        let photo = NewPhoto {
            id,
            kind,
            workspace_id: location.workspace_id,
            gallery_id: location.gallery_id,
            original_name: "../private\\photo\r\n.jpg".to_owned(),
            size: NonZeroU64::new(1024).unwrap(),
            dimensions: PhotoDimensions {
                width: NonZeroU32::new(20).unwrap(),
                height: NonZeroU32::new(10).unwrap(),
            },
        };
        let tests = database.repository::<TestRepository>();

        database
            .repository::<PhotoIntake>()
            .try_accept(photo)
            .await
            .unwrap()
            .unwrap();
        tests.force_photo_status(id, &status).await.unwrap();
        tests
            .set_photo_created_at(id, OffsetDateTime::from_unix_timestamp(created_at).unwrap())
            .await
            .unwrap();

        id
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn workspace_zip_snapshots_the_exact_mixed_selection(pool: PgPool) {
        let database = Database::new(pool.clone());
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;
        let (_, g1) = call(&app, post_gallery(&cookie)).await;
        let (_, g2) = call(&app, post_gallery(&cookie)).await;
        let (workspace_id, gallery_id) = gallery_of(&pool, &g1).await;
        let first = PhotoLocation {
            workspace_id,
            gallery_id,
        };
        let (_, gallery_id) = gallery_of(&pool, &g2).await;
        let second = PhotoLocation {
            workspace_id,
            gallery_id,
        };
        let proof = photo_in(&database, first, PhotoKind::Proof, PhotoStatus::Ready, 10).await;
        let final_photo =
            photo_in(&database, second, PhotoKind::Final, PhotoStatus::Ready, 20).await;
        let request = |ids: &[PhotoId]| {
            test::TestRequest::post()
                .uri("/api/v1/photos/export")
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN))
                .set_json(serde_json::json!({"photo_ids": ids}))
        };

        for invalid_count in [0, 1001] {
            assert_eq!(
                call(&app, request(&vec![proof; invalid_count])).await.0,
                StatusCode::UNPROCESSABLE_ENTITY
            );
        }

        let foreign = other_workspace(&pool).await;
        let foreign_gallery = database
            .repository::<GalleryRepository>()
            .create(foreign)
            .await
            .unwrap();
        let foreign_photo = photo_in(
            &database,
            PhotoLocation {
                workspace_id: foreign,
                gallery_id: foreign_gallery.id,
            },
            PhotoKind::Proof,
            PhotoStatus::Ready,
            30,
        )
        .await;
        assert_eq!(
            call(&app, request(&[proof, foreign_photo])).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            call(&app, request(&[proof, proof])).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            call(&app, request(&[proof, PhotoId::from(Uuid::new_v4())]))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        let body = serde_json::json!({"photo_ids": [proof, final_photo]}).to_string();
        let raw_request = |content_type, payload: String| {
            test::TestRequest::post()
                .uri("/api/v1/photos/export")
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN))
                .insert_header(("Content-Type", content_type))
                .set_payload(payload)
        };
        assert_eq!(
            call(&app, raw_request("text/plain", body.clone())).await.0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
        assert_eq!(
            call(
                &app,
                raw_request("application/json", format!("{}{body}", " ".repeat(65536)))
            )
            .await
            .0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        let (status, export) = call(
            &app,
            raw_request("application/json", format!("{}{body}", " ".repeat(4096))),
        )
        .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(export["photo_count"], 2);
        assert_eq!(
            call(&app, request(&[proof, final_photo])).await.0,
            StatusCode::CONFLICT
        );
        let (status, latest) = call(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/photos/export/{}",
                    export["id"].as_str().unwrap()
                ))
                .cookie(cookie),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(latest["id"], export["id"]);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_photos_requires_authentication(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        let (status, body) = call(&app, test::TestRequest::get().uri("/api/v1/photos")).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "unauthorized");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_photos_is_scoped_to_the_session_workspace_and_cached_no_store(pool: PgPool) {
        let foreign_workspace = other_workspace(&pool).await;
        let database = Database::new(pool.clone());
        let foreign_gallery = database
            .repository::<GalleryRepository>()
            .create(foreign_workspace)
            .await
            .unwrap();

        photo_in(
            &database,
            PhotoLocation {
                workspace_id: foreign_workspace,
                gallery_id: foreign_gallery.id,
            },
            PhotoKind::Proof,
            PhotoStatus::Ready,
            999,
        )
        .await;

        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        let (status, body) = call(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/photos")
                .cookie(cookie.clone())
                .insert_header(("Origin", ORIGIN)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!([]));

        let request = test::TestRequest::get()
            .uri("/api/v1/photos")
            .cookie(cookie)
            .insert_header(("Origin", ORIGIN))
            .to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_photos_returns_the_documented_row_shape_newest_first_excluding_unready(
        pool: PgPool,
    ) {
        let (state, _directory) = app_state(pool.clone()).await;
        let database = Database::new(pool.clone());
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let cookie = sign_up(&app).await;

        let (status, created) = call(&app, post_gallery(&cookie)).await;
        assert_eq!(status, StatusCode::CREATED);
        let (workspace_id, gallery_id) = gallery_of(&pool, &created).await;
        let location = PhotoLocation {
            workspace_id,
            gallery_id,
        };

        let older = photo_in(
            &database,
            location,
            PhotoKind::Proof,
            PhotoStatus::Ready,
            10,
        )
        .await;
        let newer = photo_in(
            &database,
            location,
            PhotoKind::Final,
            PhotoStatus::Ready,
            20,
        )
        .await;

        for (kind, status) in [
            (PhotoKind::Proof, PhotoStatus::Waiting),
            (PhotoKind::Proof, PhotoStatus::Processing),
            (PhotoKind::Final, PhotoStatus::Failed("broken".to_owned())),
        ] {
            photo_in(&database, location, kind, status, 30).await;
        }

        let (status, body) = call(
            &app,
            test::TestRequest::get()
                .uri("/api/v1/photos")
                .cookie(cookie)
                .insert_header(("Origin", ORIGIN)),
        )
        .await;

        assert_eq!(status, StatusCode::OK);

        let rows = body.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], serde_json::json!(newer));
        assert_eq!(rows[0]["kind"], "final");
        assert_eq!(rows[1]["id"], serde_json::json!(older));
        assert_eq!(rows[1]["kind"], "proof");

        for row in rows {
            assert_eq!(row["gallery_id"], serde_json::json!(gallery_id));
            assert_eq!(row["gallery_title"], Value::Null);
            assert!(row.get("revision").is_some());
            assert!(row.get("width").is_some());
            assert!(row.get("height").is_some());
            assert!(row.get("created_at").is_some());
            assert_eq!(row["original_name"], "photo.jpg");

            for forbidden in ["original_path", "share_token", "access_code", "size"] {
                assert!(
                    row.get(forbidden).is_none(),
                    "row must not expose {forbidden}"
                );
            }
        }
    }
}
