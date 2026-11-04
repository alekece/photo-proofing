//! Workspace-scoped photo uploads, listing, retries and derivatives.
//!
//! Uploads check workspace access and request limits before consuming the body, stage bytes in
//! storage, inspect only JPEG metadata, then publish the unchanged original and atomically record
//! its photo/job pair. Listing checks gallery ownership before returning its ordered photo states.
//! Retries requeue failed photos. Deleting a photo is accepted whatever its status and removes
//! its jobs, then its files. Only the thumbnail and preview of a ready photo are served;
//! no route reads the original.

use actix_web::{
    HttpRequest, HttpResponse, delete, get, post,
    web::{self, Data, Path as WebPath, ServiceConfig},
};
use pp_core::{notifications::PhotoId, types::generate::Generate};
use pp_database::{
    gallery::{GalleryId, GalleryRepository},
    photo::{NewPhoto, PhotoLocation, PhotoRepository, PhotoStatus},
    photo_intake::{PhotoIntake, PhotoIntakeError},
    photo_retry::PhotoRetry,
};
use pp_storage::{PhotoKey, StorageError, Variant};
use serde::Serialize;

use super::DerivativePhoto;
use crate::{
    api::ApiState,
    error::ErrorResponse,
    extractors::identity::Authenticated,
    response::{ImageFile, ResponseBuilderExt},
    upload::{PhotoUpload, limiter::UploadSlot},
};

/// Registers photo routes inside the `/api/v1/galleries` scope.
pub fn configure(cfg: &mut ServiceConfig) {
    // The literal `retry` segment is registered before the `{photo_id}` routes it would match.
    cfg.service(upload_photo)
        .service(list_photos)
        .service(retry_gallery)
        .service(retry_photo)
        .service(delete_photo)
        .service(serve_derivative_photo);
}

#[derive(Serialize)]
struct Requeued {
    requeued: u64,
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

#[post("/{id}/photos/retry")]
async fn retry_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    id: WebPath<GalleryId>,
) -> Result<HttpResponse, ErrorResponse> {
    let requeued = state
        .database()
        .repository::<PhotoRetry>()
        .retry_failed(identity.workspace_id, id.into_inner(), None)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(Requeued { requeued }))
}

#[post("/{id}/photos/{photo_id}/retry")]
async fn retry_photo(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, PhotoId)>,
) -> Result<HttpResponse, ErrorResponse> {
    let (gallery_id, photo_id) = path.into_inner();
    let requeued = state
        .database()
        .repository::<PhotoRetry>()
        .retry_failed(identity.workspace_id, gallery_id, Some(photo_id))
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok().no_store().json(Requeued { requeued }))
}

/// Deletes a photo whatever its status, then its files. The deletion stands once committed:
/// a file that cannot be removed is only logged.
#[delete("/{id}/photos/{photo_id}")]
async fn delete_photo(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, PhotoId)>,
) -> Result<HttpResponse, ErrorResponse> {
    let (gallery_id, photo_id) = path.into_inner();

    if state
        .database()
        .repository::<PhotoRepository>()
        .delete(identity.workspace_id, gallery_id, &[photo_id])
        .await?
        .is_empty()
    {
        return Err(ErrorResponse::not_found());
    }

    let location = PhotoLocation {
        workspace_id: identity.workspace_id,
        gallery_id,
    };

    for variant in [Variant::Original, Variant::Thumbnail, Variant::Preview] {
        match state
            .storage()
            .remove(&PhotoKey::with_location(location, photo_id, variant))
            .await
        {
            Ok(()) | Err(StorageError::NotFound) => {}
            Err(error) => {
                tracing::error!(%error, %photo_id, %variant, "failed to remove a deleted photo's file");
            }
        }
    }

    Ok(HttpResponse::NoContent().no_store().finish())
}

/// Serves a derivative of a ready photo; anything else, including a file removed meanwhile,
/// is not found.
#[get("/{id}/photos/{photo_id}/{derivative:thumbnail|preview}")]
async fn serve_derivative_photo(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, PhotoId, DerivativePhoto)>,
) -> Result<ImageFile, ErrorResponse> {
    let (gallery_id, photo_id, derivative) = path.into_inner();
    let photo = state
        .database()
        .repository::<PhotoRepository>()
        .get(identity.workspace_id, gallery_id, photo_id)
        .await?;

    if !matches!(photo, Some(photo) if photo.status == PhotoStatus::Ready) {
        return Err(ErrorResponse::not_found());
    }

    let location = PhotoLocation {
        workspace_id: identity.workspace_id,
        gallery_id,
    };

    ImageFile::read(
        state.storage(),
        &PhotoKey::with_location(location, photo_id, derivative.into()),
    )
    .await
}

#[post("/{id}/photos")]
async fn upload_photo(
    state: Data<ApiState>,
    _slot: UploadSlot,
    Authenticated(identity): Authenticated,
    request: HttpRequest,
    id: WebPath<GalleryId>,
    payload: web::Payload,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();
    let storage = state.storage();

    if state
        .database()
        .repository::<GalleryRepository>()
        .get(identity.workspace_id, gallery_id)
        .await?
        .is_none()
    {
        return Err(ErrorResponse::not_found());
    }

    let photo_upload = PhotoUpload::from_request(
        &request,
        payload.into_inner(),
        storage,
        state.upload_config(),
    )
    .await?;
    let photo_id = PhotoId::generate();
    let location = PhotoLocation {
        workspace_id: identity.workspace_id,
        gallery_id,
    };
    let key = PhotoKey::with_location(location, photo_id, Variant::Original);

    photo_upload.staged.commit(&key).await?;

    let new_photo = NewPhoto {
        id: photo_id,
        workspace_id: identity.workspace_id,
        gallery_id,
        original_name: photo_upload.original_name,
        size: photo_upload.size,
        dimensions: photo_upload.dimensions,
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
            match storage.remove(&key).await {
                Ok(()) | Err(StorageError::NotFound) => {}
                Err(error) => return Err(error.into()),
            }

            return Err(ErrorResponse::not_found());
        }
        Err(error @ PhotoIntakeError::Database(_)) => {
            tracing::error!(%error, photo_id = %photo_id, "photo metadata transaction failed");

            match storage.remove(&key).await {
                Ok(()) | Err(StorageError::NotFound) => {}
                Err(error) => return Err(error.into()),
            }

            return Err(ErrorResponse::unavailable());
        }
        Err(error @ PhotoIntakeError::CommitOutcomeUnknown { .. }) => {
            tracing::error!(
                %error,
                photo_id = %photo_id,
                "photo commit outcome is unknown; preserving published original"
            );

            return Err(ErrorResponse::unavailable());
        }
    };

    Ok(HttpResponse::Created().no_store().json(photo))
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        fs,
        num::{NonZeroU64, NonZeroUsize},
        path::{Path, PathBuf},
        time::Duration,
    };

    use actix_web::{
        App,
        cookie::Cookie,
        dev::{Service, ServiceResponse},
        http::{
            StatusCode,
            header::{CONTENT_LENGTH, RETRY_AFTER},
        },
        test,
        web::Data,
    };
    use futures_util::stream;
    use libvips::ops::{black, jpegsave_buffer};
    use pp_core::{config::PhotoDir, notifications::PhotoId};
    use pp_database::{
        Database,
        gallery::{GalleryId, GalleryRepository},
        jobs::{JobKind, JobRepository, JobState, PhotoResult, PhotoUpdate, ProcessPhoto},
        photo::{PhotoRepository, PhotoStatus},
        tests::TestRepository,
    };
    use pp_storage::PhotoStorage;
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use crate::{
        api::configure_routes,
        config::UploadConfig,
        tests::{ORIGIN, call, other_workspace, sign_up, state_with_upload},
        upload,
    };

    pub(crate) fn upload_config(max_size: u64, max_pixels: u64) -> UploadConfig {
        UploadConfig {
            max_size: NonZeroU64::new(max_size).unwrap(),
            max_pixels: NonZeroU64::new(max_pixels).unwrap(),
            concurrency: NonZeroUsize::new(4).unwrap(),
        }
    }

    pub(crate) async fn app(
        pool: PgPool,
        photo_dir: &Path,
        config: UploadConfig,
    ) -> impl Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>
    {
        let photo_dir: PhotoDir = photo_dir.to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();
        upload::jpeg::install().unwrap();

        test::init_service(
            App::new()
                .app_data(Data::new(upload::limiter::UploadLimiter::new(
                    config.concurrency,
                )))
                .app_data(Data::new(state_with_upload(pool, storage, config)))
                .configure(configure_routes),
        )
        .await
    }

    pub(crate) fn jpeg(width: i32, height: i32) -> Vec<u8> {
        let image = black(width, height).unwrap();

        jpegsave_buffer(&image).unwrap()
    }

    pub(crate) async fn create_gallery<S>(app: &S, cookie: &Cookie<'static>) -> String
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

    pub(crate) fn upload_request(
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

    pub(crate) async fn call_request<S>(
        app: &S,
        request: actix_http::Request,
    ) -> (StatusCode, Value)
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
        let database = Database::new(pool.clone());
        let testing = database.repository::<TestRepository>();

        assert_eq!(testing.count_photos().await.unwrap(), 0);
        assert_eq!(testing.count_jobs(None).await.unwrap(), 0);
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
        let busy = test::call_service(&app, request).await;

        assert_eq!(busy.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(busy.headers().get(RETRY_AFTER).unwrap(), "1");
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
            let jobs = Database::new(pool.clone())
                .repository::<TestRepository>()
                .photo_jobs(photo_id(id))
                .await
                .unwrap();

            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].payload["kind"], "process_photo");
            assert_eq!(jobs[0].payload["payload"]["photo_id"], id);
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

        Database::new(pool.clone())
            .repository::<TestRepository>()
            .delete_gallery(GalleryId(Uuid::parse_str(&gallery_id).unwrap()))
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

        Database::new(pool.clone())
            .repository::<TestRepository>()
            .reject_job_inserts()
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
    async fn unknown_database_commit_preserves_published_original(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let original = jpeg(1, 1);

        Database::new(pool.clone())
            .repository::<TestRepository>()
            .interrupt_photo_commits()
            .await
            .unwrap();

        let request = upload_request(
            &cookie,
            &gallery_id,
            "proof.jpg",
            "image/jpeg",
            &original,
            None,
        );
        let (status, _) = call_request(&app, request).await;
        let files = stored_files(directory.path());

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(files.len(), 1);
        assert_eq!(fs::read(&files[0]).unwrap(), original);
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
            ("late.jpg", 30_i64, PhotoStatus::Ready, json!("ready")),
            (
                "larger-id.jpg",
                20,
                PhotoStatus::Failed("processing failed".to_owned()),
                json!({"failed": "processing failed"}),
            ),
            ("oldest.jpg", 10, PhotoStatus::Waiting, json!("waiting")),
            (
                "smaller-id.jpg",
                20,
                PhotoStatus::Processing,
                json!("processing"),
            ),
        ];
        let mut expected = Vec::new();

        for (filename, created_at, status, status_json) in fixtures {
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
            let database = Database::new(pool.clone());
            let testing = database.repository::<TestRepository>();

            testing
                .force_photo_status(PhotoId::from(id), &status)
                .await
                .unwrap();
            testing
                .set_photo_created_at(
                    PhotoId::from(id),
                    OffsetDateTime::from_unix_timestamp(created_at).unwrap(),
                )
                .await
                .unwrap();
            expected.push((created_at, id, status_json, filename));
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

        for (photo, (_, id, status, filename)) in photos.iter().zip(expected.iter()) {
            assert_eq!(photo["id"], id.to_string());
            assert_eq!(photo["original_name"], *filename);
            assert_eq!(photo["size"], original.len());
            assert_eq!(photo["width"], 2);
            assert_eq!(photo["height"], 1);
            assert!(photo["created_at"].as_str().is_some());

            assert_eq!(photo["status"], *status);
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

    /// How far a test photo got through processing.
    #[derive(Clone, Copy, Debug)]
    pub(crate) enum Progress {
        Processing,
        Ready,
        Failed,
    }

    /// Uploads a photo, which then waits for processing, returning its id.
    pub(crate) async fn waiting_photo<S>(
        app: &S,
        cookie: &Cookie<'static>,
        gallery_id: &str,
    ) -> String
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = upload_request(cookie, gallery_id, "a.jpg", "image/jpeg", &jpeg(2, 1), None);
        let (status, photo) = call_request(app, request).await;

        assert_eq!(status, StatusCode::CREATED);

        photo["id"].as_str().unwrap().to_owned()
    }

    /// Uploads a photo and moves it to `progress` as the worker would, returning its id.
    ///
    /// Claims the oldest queued job, so earlier photos must not still be waiting.
    pub(crate) async fn photo_in<S>(
        app: &S,
        pool: &PgPool,
        cookie: &Cookie<'static>,
        gallery_id: &str,
        progress: Progress,
    ) -> String
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let id = waiting_photo(app, cookie, gallery_id).await;
        let photo_id = photo_id(&id);
        let database = Database::new(pool.clone());
        let jobs = database.repository::<JobRepository>();
        let job = jobs
            .claim(&[JobKind::ProcessPhoto], Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();

        assert_eq!(job.payload::<ProcessPhoto>().unwrap().photo_id, photo_id);

        let result = match progress {
            Progress::Processing => {
                let photos = database.repository::<PhotoRepository>();

                assert!(photos.mark_processing(photo_id).await.unwrap());

                return id;
            }
            Progress::Ready => PhotoResult::Ready,
            Progress::Failed => PhotoResult::Failed("The image could not be read."),
        };
        let (completion, update) = jobs.complete_photo(&job, photo_id, result).await.unwrap();

        assert!(completion.is_current());
        assert_eq!(update, PhotoUpdate::Saved);

        id
    }

    fn photo_id(id: &str) -> PhotoId {
        PhotoId::from(Uuid::parse_str(id).unwrap())
    }

    /// Writes a derivative file where the worker would store it.
    pub(crate) async fn store_derivative(
        pool: &PgPool,
        photo_dir: &Path,
        photo_id: &str,
        variant: &str,
    ) -> Vec<u8> {
        let location = Database::new(pool.clone())
            .repository::<PhotoRepository>()
            .locate(self::photo_id(photo_id))
            .await
            .unwrap()
            .unwrap();
        let bytes = format!("{variant} bytes").into_bytes();

        fs::write(
            photo_dir
                .join(location.workspace_id.0.to_string())
                .join(location.gallery_id.0.to_string())
                .join(format!("{photo_id}-{variant}")),
            &bytes,
        )
        .unwrap();

        bytes
    }

    async fn photo_status(pool: &PgPool, id: &str) -> PhotoStatus {
        let database = Database::new(pool.clone());
        let photos = database.repository::<PhotoRepository>();
        let location = photos.locate(photo_id(id)).await.unwrap().unwrap();

        photos
            .get(location.workspace_id, location.gallery_id, photo_id(id))
            .await
            .unwrap()
            .unwrap()
            .status
    }

    async fn queued_jobs(pool: &PgPool, id: &str) -> usize {
        Database::new(pool.clone())
            .repository::<TestRepository>()
            .photo_jobs(photo_id(id))
            .await
            .unwrap()
            .iter()
            .filter(|job| job.state == JobState::Queued)
            .count()
    }

    fn retry_request(cookie: Option<&Cookie<'static>>, path: &str) -> actix_http::Request {
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{path}"))
            .insert_header(("Origin", ORIGIN));

        match cookie {
            Some(cookie) => request.cookie(cookie.clone()).to_request(),
            None => request.to_request(),
        }
    }

    pub(crate) fn get_request(cookie: Option<&Cookie<'static>>, path: &str) -> actix_http::Request {
        let request = test::TestRequest::get().uri(&format!("/api/v1/galleries/{path}"));

        match cookie {
            Some(cookie) => request.cookie(cookie.clone()).to_request(),
            None => request.to_request(),
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn retry_requeues_failed_photos_and_reports_the_count(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let failed = [
            photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await,
            photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await,
        ];
        let ready = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        let (status, body) = call_request(
            &app,
            retry_request(Some(&cookie), &format!("{gallery_id}/photos/retry")),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({"requeued": 2}));

        for id in &failed {
            assert_eq!(photo_status(&pool, id).await, PhotoStatus::Waiting);
            assert_eq!(queued_jobs(&pool, id).await, 1);
        }

        assert_eq!(photo_status(&pool, &ready).await, PhotoStatus::Ready);

        let (_, body) = call_request(
            &app,
            retry_request(Some(&cookie), &format!("{gallery_id}/photos/retry")),
        )
        .await;

        assert_eq!(body, serde_json::json!({"requeued": 0}));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn retry_one_photo_only_touches_that_photo(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let target = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;
        let other = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;

        let (status, body) = call_request(
            &app,
            retry_request(
                Some(&cookie),
                &format!("{gallery_id}/photos/{target}/retry"),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, serde_json::json!({"requeued": 1}));
        assert_eq!(photo_status(&pool, &target).await, PhotoStatus::Waiting);
        assert!(matches!(
            photo_status(&pool, &other).await,
            PhotoStatus::Failed(_)
        ));
        assert_eq!(queued_jobs(&pool, &other).await, 0);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn retry_requires_authentication_and_ownership(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await;
        let foreign_gallery = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap()
            .id
            .0;

        for path in [
            format!("{gallery_id}/photos/retry"),
            format!("{gallery_id}/photos/{photo}/retry"),
        ] {
            let (status, _) = call_request(&app, retry_request(None, &path)).await;

            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        }

        for path in [
            format!("{foreign_gallery}/photos/retry"),
            format!("{foreign_gallery}/photos/{photo}/retry"),
            format!("{gallery_id}/photos/{}/retry", Uuid::new_v4()),
        ] {
            let (status, _) = call_request(&app, retry_request(Some(&cookie), &path)).await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        }

        assert!(matches!(
            photo_status(&pool, &photo).await,
            PhotoStatus::Failed(_)
        ));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn ready_photo_serves_thumbnail_and_preview(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        for variant in ["thumbnail", "preview"] {
            let stored = store_derivative(&pool, directory.path(), &photo, variant).await;
            let response = test::call_service(
                &app,
                get_request(
                    Some(&cookie),
                    &format!("{gallery_id}/photos/{photo}/{variant}"),
                ),
            )
            .await;

            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers().get("Content-Type").unwrap(),
                "image/jpeg"
            );
            assert_eq!(
                response.headers().get("Cache-Control").unwrap(),
                "private, max-age=3600"
            );
            assert_eq!(
                response.headers().get("Referrer-Policy").unwrap(),
                "no-referrer"
            );
            assert_eq!(
                response.headers().get("X-Content-Type-Options").unwrap(),
                "nosniff"
            );
            assert_eq!(test::read_body(response).await, stored);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn serving_returns_404_for_waiting_processing_and_failed_photos(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;

        // The waiting photo comes last: its queued job would otherwise be claimed first.
        let photos = [
            photo_in(&app, &pool, &cookie, &gallery_id, Progress::Processing).await,
            photo_in(&app, &pool, &cookie, &gallery_id, Progress::Failed).await,
            waiting_photo(&app, &cookie, &gallery_id).await,
        ];

        for photo in photos {
            store_derivative(&pool, directory.path(), &photo, "thumbnail").await;

            let (status, _) = call_request(
                &app,
                get_request(
                    Some(&cookie),
                    &format!("{gallery_id}/photos/{photo}/thumbnail"),
                ),
            )
            .await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{photo}");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn serving_returns_404_across_galleries_and_workspaces(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        store_derivative(&pool, directory.path(), &photo, "thumbnail").await;

        let foreign_gallery = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap()
            .id
            .0;

        for gallery in [other_gallery, foreign_gallery.to_string()] {
            let (status, _) = call_request(
                &app,
                get_request(
                    Some(&cookie),
                    &format!("{gallery}/photos/{photo}/thumbnail"),
                ),
            )
            .await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{gallery}");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn serving_requires_authentication(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        store_derivative(&pool, directory.path(), &photo, "thumbnail").await;

        let (status, _) = call_request(
            &app,
            get_request(None, &format!("{gallery_id}/photos/{photo}/thumbnail")),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn no_route_serves_the_original(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        for path in [
            "original",
            "Original",
            "download",
            "",
            "thumbnail/../original",
            "retry",
        ] {
            // Without a session too, so the path's existence is not revealed.
            for cookie in [Some(&cookie), None] {
                let response = test::call_service(
                    &app,
                    get_request(cookie, &format!("{gallery_id}/photos/{photo}/{path}")),
                )
                .await;

                assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
            }
        }
    }

    fn delete_request(cookie: Option<&Cookie<'static>>, path: &str) -> actix_http::Request {
        let request = test::TestRequest::delete()
            .uri(&format!("/api/v1/galleries/{path}"))
            .insert_header(("Origin", ORIGIN));

        match cookie {
            Some(cookie) => request.cookie(cookie.clone()).to_request(),
            None => request.to_request(),
        }
    }

    /// Lists the files stored for a photo, whatever their variant.
    fn photo_files(photo_dir: &Path, photo_id: &str) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut directories = vec![photo_dir.to_path_buf()];

        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();

                if path.is_dir() {
                    directories.push(path);
                } else if path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(photo_id)
                {
                    files.push(path);
                }
            }
        }

        files
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn delete_removes_the_photo_its_jobs_and_its_files_whatever_its_status(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let mut photos = Vec::new();

        for progress in [Progress::Processing, Progress::Ready, Progress::Failed] {
            photos.push(photo_in(&app, &pool, &cookie, &gallery_id, progress).await);
        }

        photos.push(waiting_photo(&app, &cookie, &gallery_id).await);

        for photo in &photos {
            for variant in ["thumbnail", "preview"] {
                store_derivative(&pool, directory.path(), photo, variant).await;
            }

            assert_eq!(photo_files(directory.path(), photo).len(), 3);

            let response = test::call_service(
                &app,
                delete_request(Some(&cookie), &format!("{gallery_id}/photos/{photo}")),
            )
            .await;

            assert_eq!(response.status(), StatusCode::NO_CONTENT);
            assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
            assert!(photo_files(directory.path(), photo).is_empty());
            assert!(
                Database::new(pool.clone())
                    .repository::<TestRepository>()
                    .photo_jobs(photo_id(photo))
                    .await
                    .unwrap()
                    .is_empty()
            );
        }

        let (status, listed) = call_request(
            &app,
            get_request(Some(&cookie), &format!("{gallery_id}/photos")),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(listed, json!([]));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn delete_succeeds_when_files_are_already_gone(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = waiting_photo(&app, &cookie, &gallery_id).await;
        let path = format!("{gallery_id}/photos/{photo}");

        for file in photo_files(directory.path(), &photo) {
            fs::remove_file(file).unwrap();
        }

        let (status, _) = call_request(&app, delete_request(Some(&cookie), &path)).await;

        assert_eq!(status, StatusCode::NO_CONTENT);

        let (status, _) = call_request(&app, delete_request(Some(&cookie), &path)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn delete_requires_authentication_and_ownership(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let photo = waiting_photo(&app, &cookie, &gallery_id).await;
        let foreign_gallery = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap()
            .id
            .0;
        let (status, _) = call_request(
            &app,
            delete_request(None, &format!("{gallery_id}/photos/{photo}")),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);

        for path in [
            format!("{foreign_gallery}/photos/{photo}"),
            format!("{other_gallery}/photos/{photo}"),
            format!("{gallery_id}/photos/{}", Uuid::new_v4()),
        ] {
            let (status, _) = call_request(&app, delete_request(Some(&cookie), &path)).await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        }

        assert_eq!(photo_status(&pool, &photo).await, PhotoStatus::Waiting);
        assert_eq!(photo_files(directory.path(), &photo).len(), 1);
    }
}
