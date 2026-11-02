//! Workspace-scoped photo uploads and listing.
//!
//! Uploads check workspace access and request limits before consuming the body, stage bytes in
//! storage, inspect only JPEG metadata, then publish the unchanged original and atomically record
//! its photo/job pair. Listing checks gallery ownership before returning its ordered photo states.

use actix_web::{
    HttpRequest, HttpResponse, get,
    http::header::{CONTENT_LENGTH, CONTENT_TYPE},
    post,
    web::{self, Data, Path as WebPath, ServiceConfig},
};
use pp_core::notifications::PhotoId;
use pp_database::{
    gallery::{GalleryId, GalleryRepository},
    photo::{NewPhoto, PhotoRepository},
    photo_intake::PhotoIntake,
};
use pp_storage::{PhotoKey, Variant};
use uuid::Uuid;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt, upload::UploadSlot,
};

/// Registers photo routes inside the `/api/v1/galleries` scope.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(upload_photo).service(list_photos);
}

#[get("/{id}/photos")]
async fn list_photos(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: WebPath<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();

    if state
        .database()
        .repository::<GalleryRepository>()
        .get(identity.workspace_id, gallery_id)
        .await?
        .is_none()
    {
        return Err(ErrorResponse::not_found());
    }

    let photos = state
        .database()
        .repository::<PhotoRepository>()
        .list(identity.workspace_id, gallery_id)
        .await?;

    Ok(HttpResponse::Ok().no_store().json(photos))
}

#[post("/{id}/photos")]
async fn upload_photo(
    state: Data<ApiState>,
    slot: UploadSlot,
    Authenticated(identity): Authenticated,
    request: HttpRequest,
    id: WebPath<GalleryId>,
    payload: web::Payload,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();
    let upload = state.upload();

    if state
        .database()
        .repository::<GalleryRepository>()
        .get(identity.workspace_id, gallery_id)
        .await?
        .is_none()
    {
        return Err(ErrorResponse::not_found());
    }

    if request
        .headers()
        .get(CONTENT_TYPE)
        .is_none_or(|value| value.as_bytes() != b"image/jpeg")
    {
        return Err(ErrorResponse::unsupported_media_type());
    }

    let max_size = upload.max_size().get();

    if let Some(length) = request.headers().get(CONTENT_LENGTH) {
        let length = length
            .to_str()
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(ErrorResponse::bad_request)?;

        if length > max_size {
            return Err(ErrorResponse::payload_too_large());
        }
    }

    let (staged, size) = slot.stage_payload(payload.into_inner()).await?;
    let dimensions = upload.inspect_jpeg(&staged).await?;
    let photo_id = PhotoId::from(Uuid::new_v4());
    let key = PhotoKey {
        workspace: identity.workspace_id,
        gallery: gallery_id,
        photo: photo_id,
        variant: Variant::Original,
    };

    upload.commit_original(staged, &key).await?;

    let new_photo = NewPhoto {
        id: photo_id,
        workspace_id: identity.workspace_id,
        gallery_id,
        original_name: upload.original_name(&request),
        size,
        dimensions,
    };

    let photo = match state
        .database()
        .repository::<PhotoIntake>()
        .try_accept(new_photo)
        .await
    {
        Ok(Some(photo)) => photo,
        Ok(None) => {
            // Insertion rechecks ownership: the gallery may have disappeared while streaming.
            upload.remove_published_original(&key).await?;

            return Err(ErrorResponse::not_found());
        }
        Err(error) => {
            upload.cleanup_after_failure(&key, &error).await?;

            return Err(ErrorResponse::unavailable());
        }
    };

    Ok(HttpResponse::Created().no_store().json(photo))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        num::{NonZeroU64, NonZeroUsize},
        path::{Path, PathBuf},
    };

    use actix_web::{
        App,
        cookie::Cookie,
        dev::{Service, ServiceResponse},
        http::{StatusCode, header::CONTENT_LENGTH},
        test,
        web::Data,
    };
    use futures_util::stream;
    use libvips::ops::{black, jpegsave_buffer};
    use pp_core::config::PhotoDir;
    use pp_database::{Database, gallery::GalleryRepository};
    use pp_storage::PhotoStorage;
    use serde_json::Value;
    use sqlx::PgPool;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use crate::{
        api::configure_routes,
        config::UploadConfig,
        tests::{ORIGIN, call, other_workspace, sign_up, state_with_upload},
        upload::{self, UploadState},
    };

    fn upload_config(max_size: u64, max_pixels: u64) -> UploadConfig {
        UploadConfig {
            max_size: NonZeroU64::new(max_size).unwrap(),
            max_pixels: NonZeroU64::new(max_pixels).unwrap(),
            concurrency: NonZeroUsize::new(4).unwrap(),
        }
    }

    async fn app(
        pool: PgPool,
        photo_dir: &Path,
        config: UploadConfig,
    ) -> impl Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>
    {
        let photo_dir: PhotoDir = photo_dir.to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();
        upload::install().unwrap();
        let photo_upload = UploadState::new(storage, config);

        test::init_service(
            App::new()
                .app_data(Data::new(state_with_upload(pool, photo_upload)))
                .configure(configure_routes),
        )
        .await
    }

    fn jpeg(width: i32, height: i32) -> Vec<u8> {
        let image = black(width, height).unwrap();

        jpegsave_buffer(&image).unwrap()
    }

    async fn create_gallery<S>(app: &S, cookie: &Cookie<'static>) -> String
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::post()
            .uri("/api/v1/galleries")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone());
        let (status, gallery) = call(app, request).await;

        assert_eq!(status, StatusCode::CREATED);

        gallery["id"].as_str().unwrap().to_owned()
    }

    fn upload_request(
        cookie: &Cookie<'static>,
        gallery_id: &str,
        filename: &str,
        content_type: &str,
        body: &[u8],
        declared_length: Option<usize>,
    ) -> actix_http::Request {
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos"))
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", content_type))
            .insert_header(("X-Filename", filename))
            .cookie(cookie.clone())
            .set_payload(body.to_vec());

        if let Some(length) = declared_length {
            request.insert_header((CONTENT_LENGTH, length)).to_request()
        } else {
            let mut request = request.to_request();
            request.headers_mut().remove(CONTENT_LENGTH);

            request
        }
    }

    async fn call_request<S>(app: &S, request: actix_http::Request) -> (StatusCode, Value)
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let response = test::call_service(app, request).await;
        let status = response.status();
        let bytes = test::read_body(response).await;
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

        (status, body)
    }

    fn stored_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut directories = vec![root.to_path_buf()];

        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();

                if path.is_dir() {
                    directories.push(path);
                } else {
                    files.push(path);
                }
            }
        }

        files
    }

    async fn assert_no_upload_effects(pool: &PgPool, photo_dir: &Path) {
        let photo_count: i64 = sqlx::query_scalar("SELECT count(*) FROM photos")
            .fetch_one(pool)
            .await
            .unwrap();
        let job_count: i64 = sqlx::query_scalar("SELECT count(*) FROM jobs")
            .fetch_one(pool)
            .await
            .unwrap();

        assert_eq!(photo_count, 0);
        assert_eq!(job_count, 0);
        assert!(stored_files(photo_dir).is_empty());
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn eager_slot_is_held_during_streaming_and_released_on_cancellation(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let mut config = upload_config(1024, 100);
        config.concurrency = NonZeroUsize::new(1).unwrap();
        let app = app(pool.clone(), directory.path(), config).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let (body_started, body_started_rx) = oneshot::channel();
        let (_release_body, release_body_rx) = oneshot::channel::<()>();
        let body = stream::once(async move {
            body_started.send(()).unwrap();
            release_body_rx.await.unwrap();

            Ok::<_, actix_http::error::PayloadError>(actix_web::web::Bytes::new())
        });
        let body: actix_http::BoxedPayloadStream = Box::pin(body);
        let request = upload_request(&cookie, &gallery_id, "proof.jpg", "image/jpeg", &[], None);
        let (request, _) = request.replace_payload(body.into());
        let mut response = Box::pin(test::call_service(&app, request));

        tokio::select! {
            started = body_started_rx => started.unwrap(),
            response = &mut response => panic!("upload completed early: {}", response.status()),
        }

        let request = upload_request(&cookie, &gallery_id, "proof.jpg", "text/plain", &[], None);
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        drop(response);
        assert_no_upload_effects(&pool, directory.path()).await;

        let request = upload_request(&cookie, &gallery_id, "proof.jpg", "text/plain", &[], None);
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        let request = upload_request(
            &cookie,
            &gallery_id,
            "proof.jpg",
            "image/jpeg",
            &jpeg(1, 1),
            None,
        );
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::CREATED);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn failed_authentication_does_not_consume_body_or_leak_slot(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let mut config = upload_config(1024, 100);
        config.concurrency = NonZeroUsize::new(1).unwrap();
        let app = app(pool.clone(), directory.path(), config).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let body = stream::poll_fn(
            |_| -> std::task::Poll<
                Option<Result<actix_web::web::Bytes, actix_http::error::PayloadError>>,
            > {
                panic!("unauthenticated request polled its body");
            },
        );
        let body: actix_http::BoxedPayloadStream = Box::pin(body);
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos"))
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", "image/jpeg"))
            .to_request();
        let (request, _) = request.replace_payload(body.into());
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_no_upload_effects(&pool, directory.path()).await;
        let request = upload_request(
            &cookie,
            &gallery_id,
            "proof.jpg",
            "image/jpeg",
            &jpeg(1, 1),
            None,
        );
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::CREATED);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn accepted_upload_preserves_jpeg_and_enqueues_job(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let original = jpeg(2, 1);
        let request = upload_request(
            &cookie,
            &gallery_id,
            "proof%20one.jpg",
            "image/jpeg",
            &original,
            None,
        );
        let (status, first) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(first["original_name"], "proof one.jpg");
        assert_eq!(first["size"], original.len());
        assert_eq!(first["width"], 2);
        assert_eq!(first["height"], 1);
        assert_eq!(first["status"], "waiting");

        let first_id = first["id"].as_str().unwrap();
        let files = stored_files(directory.path());

        assert_eq!(files.len(), 1);
        assert_eq!(fs::read(&files[0]).unwrap(), original);

        let duplicate = upload_request(
            &cookie,
            &gallery_id,
            "proof%20one.jpg",
            "image/jpeg",
            &original,
            None,
        );
        let (duplicate_status, second) = call_request(&app, duplicate).await;

        assert_eq!(duplicate_status, StatusCode::CREATED);
        assert_ne!(first_id, second["id"].as_str().unwrap());
        assert_eq!(second["original_name"], "proof one.jpg");
        assert_eq!(stored_files(directory.path()).len(), 2);

        for id in [first_id, second["id"].as_str().unwrap()] {
            let payload: Value = sqlx::query_scalar(
                "SELECT payload FROM jobs WHERE payload->'payload'->>'photo_id' = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();

            assert_eq!(payload["kind"], "process_photo");
            assert_eq!(payload["payload"]["photo_id"], id);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn rejected_uploads_leave_no_file_photo_or_job(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(512, 1)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let original = jpeg(2, 1);
        let oversized = vec![0; 513];
        let cases = [
            (
                "image/png",
                "proof.jpg",
                original.as_slice(),
                None,
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (
                "image/jpeg",
                "proof.jpg",
                original.as_slice(),
                Some(513),
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
            (
                "image/jpeg",
                "proof.jpg",
                oversized.as_slice(),
                Some(1),
                StatusCode::PAYLOAD_TOO_LARGE,
            ),
            (
                "image/jpeg",
                "proof.jpg",
                b"not a jpeg".as_slice(),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                "image/jpeg",
                "proof.jpg",
                b"\xff\xd8".as_slice(),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (
                "image/jpeg",
                "proof.jpg",
                original.as_slice(),
                None,
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
        ];

        for (content_type, filename, body, declared_length, expected) in cases {
            let request = upload_request(
                &cookie,
                &gallery_id,
                filename,
                content_type,
                body,
                declared_length,
            );
            let (status, _) = call_request(&app, request).await;

            assert_eq!(status, expected, "{content_type}, {filename}");
            assert_no_upload_effects(&pool, directory.path()).await;
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn truncated_sos_header_is_rejected_without_side_effects(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let original = jpeg(2, 1);
        let sos_marker = original
            .windows(2)
            .position(|marker| marker == [0xff, 0xda])
            .unwrap();
        let truncated = &original[..sos_marker + 6];
        let request = upload_request(
            &cookie,
            &gallery_id,
            "truncated.jpg",
            "image/jpeg",
            truncated,
            None,
        );

        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_no_upload_effects(&pool, directory.path()).await;
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn invalid_filename_falls_back_to_generic_name(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let original = jpeg(1, 1);

        let long_filename = "x".repeat(256);

        for filename in ["bad%00name.jpg", "bad%FFname.jpg", &long_filename] {
            let request = upload_request(
                &cookie,
                &gallery_id,
                filename,
                "image/jpeg",
                &original,
                None,
            );
            let (status, photo) = call_request(&app, request).await;

            assert_eq!(status, StatusCode::CREATED);
            assert_eq!(photo["original_name"], "photo.jpg");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn another_workspace_cannot_upload_to_a_gallery(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let foreign_workspace = other_workspace(&pool).await;
        let foreign_gallery = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(foreign_workspace)
            .await
            .unwrap();
        let request = upload_request(
            &cookie,
            &foreign_gallery.id.0.to_string(),
            "proof.jpg",
            "image/png",
            b"not read before authorization",
            None,
        );
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_no_upload_effects(&pool, directory.path()).await;
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn deleting_gallery_while_upload_body_is_streaming_removes_published_original(
        pool: PgPool,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let original = jpeg(1, 1);
        let body_length = original.len();
        let (body_started, body_started_rx) = oneshot::channel();
        let (release_body, release_body_rx) = oneshot::channel();
        let body = stream::once(async move {
            body_started.send(()).unwrap();
            release_body_rx.await.unwrap();

            Ok::<_, actix_http::error::PayloadError>(actix_web::web::Bytes::from(original))
        });
        let body: actix_http::BoxedPayloadStream = Box::pin(body);
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos"))
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", "image/jpeg"))
            .insert_header(("X-Filename", "proof.jpg"))
            .insert_header((CONTENT_LENGTH, body_length))
            .cookie(cookie)
            .to_request();
        let (request, _) = request.replace_payload(body.into());
        let response = test::call_service(&app, request);
        tokio::pin!(response);

        tokio::select! {
            _ = body_started_rx => {}
            response = &mut response => {
                panic!("upload completed before its body was released: {}", response.status());
            }
        }

        sqlx::query("DELETE FROM galleries WHERE id = $1")
            .bind(Uuid::parse_str(&gallery_id).unwrap())
            .execute(&pool)
            .await
            .unwrap();
        release_body.send(()).unwrap();

        let response = response.await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        test::read_body(response).await;
        assert_no_upload_effects(&pool, directory.path()).await;
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn database_failure_removes_published_original(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;

        sqlx::query(
            "CREATE FUNCTION reject_job_insert() RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN RAISE EXCEPTION 'injected test failure'; END; $$",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_job_insert BEFORE INSERT ON jobs
             FOR EACH ROW EXECUTE FUNCTION reject_job_insert()",
        )
        .execute(&pool)
        .await
        .unwrap();

        let request = upload_request(
            &cookie,
            &gallery_id,
            "proof.jpg",
            "image/jpeg",
            &jpeg(1, 1),
            None,
        );
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_no_upload_effects(&pool, directory.path()).await;
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_empty_gallery_returns_empty_array(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool, directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let request = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos"))
            .cookie(cookie)
            .to_request();

        let (status, photos) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(photos, serde_json::json!([]));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_returns_all_states_in_creation_and_id_order(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let original = jpeg(2, 1);
        let fixtures = [
            ("late.jpg", 30_i64, "ready", None),
            ("larger-id.jpg", 20, "failed", Some("processing failed")),
            ("oldest.jpg", 10, "waiting", None),
            ("smaller-id.jpg", 20, "processing", None),
        ];
        let mut expected = Vec::new();

        for (filename, created_at, status, failure_reason) in fixtures {
            let request = upload_request(
                &cookie,
                &gallery_id,
                filename,
                "image/jpeg",
                &original,
                None,
            );
            let (upload_status, photo) = call_request(&app, request).await;

            assert_eq!(upload_status, StatusCode::CREATED);

            let id = Uuid::parse_str(photo["id"].as_str().unwrap()).unwrap();

            sqlx::query(
                "UPDATE photos SET status = $2::photo_status, failure_reason = $3,
                 created_at = to_timestamp($4) WHERE id = $1",
            )
            .bind(id)
            .bind(status)
            .bind(failure_reason)
            .bind(created_at)
            .execute(&pool)
            .await
            .unwrap();
            expected.push((created_at, id, status, filename, failure_reason));
        }

        expected.sort_by_key(|(created_at, id, ..)| (*created_at, *id));

        let request = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos"))
            .cookie(cookie)
            .to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");

        let body = test::read_body(response).await;
        let photos: Value = serde_json::from_slice(&body).unwrap();
        let photos = photos.as_array().unwrap();

        assert_eq!(photos.len(), expected.len());

        for (photo, (_, id, status, filename, failure_reason)) in photos.iter().zip(expected.iter())
        {
            assert_eq!(photo["id"], id.to_string());
            assert_eq!(photo["original_name"], *filename);
            assert_eq!(photo["size"], original.len());
            assert_eq!(photo["width"], 2);
            assert_eq!(photo["height"], 1);
            assert!(photo["created_at"].as_str().is_some());

            if let Some(reason) = failure_reason {
                assert_eq!(photo["status"], serde_json::json!({"failed": reason}));
            } else {
                assert_eq!(photo["status"], *status);
            }
            assert!(!photo.as_object().unwrap().contains_key("failure_reason"));
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_requires_authentication(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool, directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let request = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos"))
            .to_request();

        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn listing_foreign_workspace_gallery_returns_not_found(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let owner_cookie = sign_up(&app).await;
        let foreign_workspace = other_workspace(&pool).await;
        let foreign_gallery = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(foreign_workspace)
            .await
            .unwrap();
        let foreign_gallery_id = foreign_gallery.id.0.to_string();

        let request = test::TestRequest::get()
            .uri(&format!("/api/v1/galleries/{foreign_gallery_id}/photos"))
            .cookie(owner_cookie)
            .to_request();
        let (status, body) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "not found");
    }
}
