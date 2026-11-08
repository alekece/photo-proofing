//! Workspace-scoped photo uploads, listing, retries and derivatives.
//!
//! Uploads check workspace access and request limits before consuming the body, stage bytes in
//! storage, inspect only JPEG metadata, then publish the unchanged original and atomically record
//! its photo/job pair. Listing checks gallery ownership before returning its ordered photo states.
//! Retries requeue failed photos. Deleting photos, one or up to a thousand at once, is accepted
//! whatever their status and removes their jobs, then their files. Only the thumbnail and preview
//! of a ready photo are served. Originals are downloadable only through photographer authentication.

use actix_web::{
    FromRequest, HttpRequest, HttpResponse,
    body::SizedStream,
    delete,
    dev::Payload,
    get,
    http::header::{self, ContentDisposition, DispositionParam, DispositionType},
    post, route,
    web::{self, Data, JsonBody, Path as WebPath, Query, ServiceConfig},
};
use futures_util::future::LocalBoxFuture;
use pp_core::types::generate::Generate;
use pp_database::photo::PhotoId;
use pp_database::{
    gallery::{GalleryId, GalleryRepository},
    photo::{NewPhoto, PhotoKind, PhotoLocation, PhotoRepository, Watermarked},
    photo_intake::{PhotoIntake, PhotoIntakeError},
    photo_retry::PhotoRetry,
    workspace_deletion::WorkspaceDeletion,
};
use pp_storage::{PhotoKey, StorageError, Variant};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;

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
    // The literal `delete` and `retry` segments are registered before the `{photo_id}` routes
    // they would match.
    cfg.service(delete_photos)
        .service(upload_photo)
        .service(list_photos)
        .service(retry_gallery)
        .service(retry_photo)
        .service(delete_photo)
        .service(download_original)
        .service(serve_derivative_photo);
}

/// Streams an unchanged proof original to its owning photographer, never through client access.
#[route("/{id}/photos/{photo_id}/download", method = "GET", method = "HEAD")]
async fn download_original(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, PhotoId)>,
    set: Query<PhotoSet>,
) -> Result<HttpResponse, ErrorResponse> {
    let (gallery_id, photo_id) = path.into_inner();

    if set.finals {
        return Err(ErrorResponse::not_found());
    }

    let photo = state
        .database()
        .repository::<PhotoRepository>()
        .get(
            identity.workspace_id,
            gallery_id,
            photo_id,
            PhotoKind::Proof,
        )
        .await?
        .ok_or_else(ErrorResponse::not_found)?;
    let key = PhotoKey {
        workspace: identity.workspace_id,
        gallery: gallery_id,
        photo: photo_id,
        variant: Variant::Original,
    };
    let file = state.storage().read(&key).await?;
    let length = file.metadata().await.map_err(StorageError::from)?.len();
    let name = photo.download_name();

    Ok(HttpResponse::Ok()
        .no_store()
        .insert_header((header::CONTENT_TYPE, "image/jpeg"))
        .insert_header((header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        .insert_header(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(name)],
        })
        .body(SizedStream::new(length, ReaderStream::new(file))))
}

/// The photo set a route addresses, chosen by the `finals` query parameter: the client proofs by
/// default, or the finished photos of a draft with `?finals=true`.
#[derive(Debug, Clone, Copy, Deserialize)]
struct PhotoSet {
    #[serde(default)]
    finals: bool,
    /// Whether the photographer wants the marked proof files: only photos ready for them are
    /// listed, and they are served. Clean when absent.
    watermark: Option<Watermarked>,
}

impl From<PhotoSet> for PhotoKind {
    fn from(set: PhotoSet) -> Self {
        if set.finals { Self::Final } else { Self::Proof }
    }
}

#[derive(Serialize)]
struct RequeuedPhotos {
    requeued: u64,
}

#[derive(Serialize)]
struct DeletedPhotos {
    deleted: usize,
}

/// Body of a bulk delete: between one and [`PhotoSelection::MAX_LEN`] photo ids.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PhotoSelection {
    photo_ids: Vec<PhotoId>,
}

impl PhotoSelection {
    /// Most photos one bulk delete accepts. Clients split larger selections by it, so it is part
    /// of the API contract rather than a deployment setting.
    const MAX_LEN: usize = 1000;

    /// Fits [`Self::MAX_LEN`] ids, at 39 bytes each, with room for whitespace.
    const JSON_LIMIT: usize = 64 * 1024;
}

/// Reads the body with its own size limit, larger than the shared JSON one, and rejects an
/// empty or oversized selection as a bad request.
impl FromRequest for PhotoSelection {
    type Error = ErrorResponse;
    type Future = LocalBoxFuture<'static, Result<Self, ErrorResponse>>;

    fn from_request(request: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let body = JsonBody::<Self>::new(request, payload, None, true).limit(Self::JSON_LIMIT);

        Box::pin(async move {
            let selection = body.await?;

            if selection.photo_ids.is_empty() || selection.photo_ids.len() > Self::MAX_LEN {
                return Err(ErrorResponse::bad_request());
            }

            Ok(selection)
        })
    }
}

#[get("/{id}/photos")]
async fn list_photos(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<GalleryId>,
    set: Query<PhotoSet>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = path.into_inner();
    let set = set.into_inner();

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
        .list(identity.workspace_id, gallery_id, set.into(), set.watermark)
        .await?;

    Ok(HttpResponse::Ok().no_store().json(photos))
}

#[post("/{id}/photos/retry")]
async fn retry_gallery(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<GalleryId>,
    set: Query<PhotoSet>,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = path.into_inner();
    let set = set.into_inner();
    let requeued = state
        .database()
        .repository_with::<PhotoRetry>(state.job_options())
        .retry_failed(identity.workspace_id, gallery_id, set.into(), None)
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok()
        .no_store()
        .json(RequeuedPhotos { requeued }))
}

#[post("/{id}/photos/{photo_id}/retry")]
async fn retry_photo(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, PhotoId)>,
    set: Query<PhotoSet>,
) -> Result<HttpResponse, ErrorResponse> {
    let (gallery_id, photo_id) = path.into_inner();
    let set = set.into_inner();
    let requeued = state
        .database()
        .repository_with::<PhotoRetry>(state.job_options())
        .retry_failed(
            identity.workspace_id,
            gallery_id,
            set.into(),
            Some(photo_id),
        )
        .await?
        .ok_or_else(ErrorResponse::not_found)?;

    Ok(HttpResponse::Ok()
        .no_store()
        .json(RequeuedPhotos { requeued }))
}

/// Deletes a photo whatever its status. Its files are removed afterwards by the worker.
#[delete("/{id}/photos/{photo_id}")]
async fn delete_photo(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, PhotoId)>,
    set: Query<PhotoSet>,
) -> Result<HttpResponse, ErrorResponse> {
    let (gallery_id, photo_id) = path.into_inner();
    let set = set.into_inner();

    if state
        .database()
        .repository_with::<WorkspaceDeletion>(state.job_options())
        .delete_photos(identity.workspace_id, gallery_id, set.into(), &[photo_id])
        .await?
        .is_empty()
    {
        return Err(ErrorResponse::not_found());
    }

    Ok(HttpResponse::NoContent().no_store().finish())
}

/// Deletes several photos of a gallery whatever their status. Ids outside the gallery or already
/// gone are ignored and not counted. Their files are removed afterwards by the worker.
#[post("/{id}/photos/delete")]
async fn delete_photos(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<GalleryId>,
    set: Query<PhotoSet>,
    selection: PhotoSelection,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = path.into_inner();
    let set = set.into_inner();

    if state
        .database()
        .repository::<GalleryRepository>()
        .get(identity.workspace_id, gallery_id)
        .await?
        .is_none()
    {
        return Err(ErrorResponse::not_found());
    }

    let removed_ids = state
        .database()
        .repository_with::<WorkspaceDeletion>(state.job_options())
        .delete_photos(
            identity.workspace_id,
            gallery_id,
            set.into(),
            &selection.photo_ids,
        )
        .await?;

    Ok(HttpResponse::Ok().no_store().json(DeletedPhotos {
        deleted: removed_ids.len(),
    }))
}

/// Serves a derivative of a ready photo; anything else, including a file removed meanwhile,
/// is not found.
#[get("/{id}/photos/{photo_id}/{derivative:thumbnail|preview}")]
async fn serve_derivative_photo(
    state: Data<ApiState>,
    Authenticated(identity): Authenticated,
    path: WebPath<(GalleryId, PhotoId, DerivativePhoto)>,
    set: Query<PhotoSet>,
) -> Result<ImageFile, ErrorResponse> {
    let (gallery_id, photo_id, derivative) = path.into_inner();
    let set = set.into_inner();
    let watermarked = set.watermark.unwrap_or_default();
    let location = PhotoLocation {
        workspace_id: identity.workspace_id,
        gallery_id,
    };

    if !state
        .database()
        .repository::<PhotoRepository>()
        .is_ready(location, photo_id, set.into(), watermarked)
        .await?
    {
        return Err(ErrorResponse::not_found());
    }

    let image = ImageFile::read(
        state.storage(),
        &PhotoKey::with_location(location, photo_id, derivative.variant(watermarked)),
    )
    .await?;

    Ok(if set.finals { image } else { image.proof() })
}

#[post("/{id}/photos")]
async fn upload_photo(
    state: Data<ApiState>,
    _slot: UploadSlot,
    Authenticated(identity): Authenticated,
    request: HttpRequest,
    path: WebPath<GalleryId>,
    set: Query<PhotoSet>,
    payload: web::Payload,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = path.into_inner();
    let set = set.into_inner();
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
        kind: set.into(),
        id: photo_id,
        workspace_id: identity.workspace_id,
        gallery_id,
        original_name: photo_upload.original_name,
        size: photo_upload.size,
        dimensions: photo_upload.dimensions,
    };

    let photo = match state
        .database()
        .repository_with::<PhotoIntake>(state.job_options())
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
pub(super) mod tests {
    use std::{
        fs,
        num::{NonZeroU32, NonZeroU64, NonZeroUsize},
        path::{Path, PathBuf},
        time::Duration,
    };

    use actix_web::{
        cookie::Cookie,
        dev::{Service, ServiceResponse},
        http::{
            StatusCode,
            header::{CONTENT_LENGTH, RETRY_AFTER},
        },
        test,
    };
    use futures_util::stream;
    use libvips::ops::{black, jpegsave_buffer};
    use pp_core::types::generate::Generate;
    use pp_database::photo::PhotoId;
    use pp_database::{
        Database,
        gallery::{GalleryId, GalleryRepository},
        jobs::{JobKind, JobRepository, JobState, PhotoResult, PhotoUpdate, ProcessPhoto},
        photo::{NewPhoto, PhotoDimensions, PhotoKind, PhotoRepository, PhotoStatus},
        photo_intake::PhotoIntake,
        tests::TestRepository,
    };
    use serde_json::{Value, json};
    use sqlx::PgPool;
    use time::OffsetDateTime;
    use tokio::sync::oneshot;
    use uuid::Uuid;

    use crate::tests::{ORIGIN, app, call, call_request, other_workspace, sign_up, upload_config};

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
        let busy_response = test::call_service(&app, request).await;

        assert_eq!(busy_response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(busy_response.headers().get(RETRY_AFTER).unwrap(), "1");
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
            .get(
                location.workspace_id,
                location.gallery_id,
                photo_id(id),
                PhotoKind::Proof,
            )
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
            assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
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
    async fn no_derivative_route_serves_the_original(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;

        for path in ["original", "Original", "", "thumbnail/../original", "retry"] {
            // Without a session too, so the path's existence is not revealed.
            for cookie in [Some(&cookie), None] {
                let response = test::call_service(
                    &app,
                    get_request(cookie, &format!("{gallery_id}/photos/{photo}/{path}")),
                )
                .await;

                // actix-web 4.18.2 answers 405 (not 404) for a known path with the wrong method.
                let status = response.status();

                assert!(
                    status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED,
                    "{path}: {status}"
                );
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
    async fn delete_removes_the_photo_and_its_jobs_and_queues_file_cleanup_whatever_its_status(
        pool: PgPool,
    ) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let mut photos = Vec::new();

        for progress in [Progress::Processing, Progress::Ready, Progress::Failed] {
            photos.push(photo_in(&app, &pool, &cookie, &gallery_id, progress).await);
        }

        photos.push(waiting_photo(&app, &cookie, &gallery_id).await);

        for (index, photo) in photos.iter().enumerate() {
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

            let database = Database::new(pool.clone());
            let tests = database.repository::<TestRepository>();
            let cleanups = tests.jobs_of_kind("cleanup_photos").await.unwrap();

            assert!(tests.photo_jobs(photo_id(photo)).await.unwrap().is_empty());
            assert_eq!(cleanups.len(), index + 1);
            assert_eq!(
                cleanups[index].payload["payload"]["photo_ids"],
                json!([photo])
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

    fn bulk_delete_request(
        cookie: Option<&Cookie<'static>>,
        gallery_id: &str,
        body: impl Into<String>,
    ) -> actix_http::Request {
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos/delete"))
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", "application/json"))
            .set_payload(body.into());

        match cookie {
            Some(cookie) => request.cookie(cookie.clone()).to_request(),
            None => request.to_request(),
        }
    }

    fn photo_ids(ids: &[impl AsRef<str>]) -> String {
        json!({ "photo_ids": ids.iter().map(AsRef::as_ref).collect::<Vec<_>>() }).to_string()
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn bulk_delete_removes_photos_and_jobs_and_queues_one_file_cleanup(pool: PgPool) {
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
        }

        let kept = photos.pop().unwrap();
        let response = test::call_service(
            &app,
            bulk_delete_request(Some(&cookie), &gallery_id, photo_ids(&photos)),
        )
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("Cache-Control").unwrap(), "no-store");
        assert_eq!(
            test::read_body_json::<Value, _>(response).await,
            json!({ "deleted": 3 })
        );

        let tests = Database::new(pool.clone());
        let tests = tests.repository::<TestRepository>();

        for photo in &photos {
            assert!(tests.photo_jobs(photo_id(photo)).await.unwrap().is_empty());
        }

        let cleanups = tests.jobs_of_kind("cleanup_photos").await.unwrap();
        let mut named = cleanups[0].payload["payload"]["photo_ids"]
            .as_array()
            .unwrap()
            .clone();
        let mut expected: Vec<_> = photos.iter().map(|photo| json!(photo)).collect();

        named.sort_by_key(ToString::to_string);
        expected.sort_by_key(ToString::to_string);

        assert_eq!(cleanups.len(), 1);
        assert_eq!(named, expected);
        assert_eq!(photo_files(directory.path(), &kept).len(), 3);
        assert_eq!(photo_status(&pool, &kept).await, PhotoStatus::Waiting);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn bulk_delete_ignores_foreign_unknown_and_repeated_ids(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let other_gallery = create_gallery(&app, &cookie).await;
        let own = waiting_photo(&app, &cookie, &gallery_id).await;
        let other = waiting_photo(&app, &cookie, &other_gallery).await;
        let unknown = Uuid::new_v4().to_string();
        let (status, body) = call_request(
            &app,
            bulk_delete_request(
                Some(&cookie),
                &gallery_id,
                photo_ids(&[&own, &own, &other, &unknown]),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "deleted": 1 }));
        assert_eq!(photo_status(&pool, &other).await, PhotoStatus::Waiting);
        assert_eq!(photo_files(directory.path(), &other).len(), 1);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn bulk_delete_leaves_another_workspace_photo_untouched(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let database = Database::new(pool.clone());
        let foreign_workspace = other_workspace(&pool).await;
        let foreign_gallery = database
            .repository::<GalleryRepository>()
            .create(foreign_workspace)
            .await
            .unwrap();
        let foreign_photo = PhotoId::generate();

        database
            .repository::<PhotoIntake>()
            .try_accept(NewPhoto {
                kind: PhotoKind::Proof,
                id: foreign_photo,
                workspace_id: foreign_workspace,
                gallery_id: foreign_gallery.id,
                original_name: "foreign.jpg".to_owned(),
                size: NonZeroU64::new(1024).unwrap(),
                dimensions: PhotoDimensions {
                    width: NonZeroU32::new(2).unwrap(),
                    height: NonZeroU32::new(1).unwrap(),
                },
            })
            .await
            .unwrap()
            .unwrap();

        let foreign_photo = foreign_photo.to_string();
        let (status, body) = call_request(
            &app,
            bulk_delete_request(Some(&cookie), &gallery_id, photo_ids(&[&foreign_photo])),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "deleted": 0 }));
        assert_eq!(
            photo_status(&pool, &foreign_photo).await,
            PhotoStatus::Waiting
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn bulk_delete_accepts_1000_ids_and_rejects_more(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let ids = |count: usize| -> Vec<String> {
            (0..count).map(|_| Uuid::new_v4().to_string()).collect()
        };
        let pretty = serde_json::to_string_pretty(&json!({ "photo_ids": ids(1000) })).unwrap();
        let (status, body) = call_request(
            &app,
            bulk_delete_request(Some(&cookie), &gallery_id, pretty),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "deleted": 0 }));

        for (body, expected) in [
            (photo_ids(&ids(1001)), StatusCode::BAD_REQUEST),
            (photo_ids(&ids(0)), StatusCode::BAD_REQUEST),
            ("not json".to_owned(), StatusCode::BAD_REQUEST),
            (
                json!({ "photo_ids": "x" }).to_string(),
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
            (photo_ids(&["not-a-uuid"]), StatusCode::UNPROCESSABLE_ENTITY),
        ] {
            let (status, _) = call_request(
                &app,
                bulk_delete_request(Some(&cookie), &gallery_id, body.clone()),
            )
            .await;

            assert_eq!(status, expected, "{body:.40}");
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn bulk_delete_requires_a_json_content_type(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = waiting_photo(&app, &cookie, &gallery_id).await;
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/photos/delete"))
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", "text/plain"))
            .cookie(cookie)
            .set_payload(photo_ids(&[&photo]))
            .to_request();
        let (status, _) = call_request(&app, request).await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(photo_status(&pool, &photo).await, PhotoStatus::Waiting);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn bulk_delete_requires_authentication_and_ownership(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let photo = waiting_photo(&app, &cookie, &gallery_id).await;
        let foreign_gallery = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap()
            .id
            .0
            .to_string();
        let (status, _) = call_request(
            &app,
            bulk_delete_request(None, &gallery_id, photo_ids(&[&photo])),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);

        for gallery in [foreign_gallery, Uuid::new_v4().to_string()] {
            let (status, _) = call_request(
                &app,
                bulk_delete_request(Some(&cookie), &gallery, photo_ids(&[&photo])),
            )
            .await;

            assert_eq!(status, StatusCode::NOT_FOUND, "{gallery}");
        }

        assert_eq!(photo_status(&pool, &photo).await, PhotoStatus::Waiting);
    }

    async fn upload_final<S>(
        app: &S,
        cookie: &Cookie<'static>,
        gallery_id: &str,
        body: &[u8],
    ) -> String
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::post()
            .uri(&format!(
                "/api/v1/galleries/{gallery_id}/photos?finals=true"
            ))
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", "image/jpeg"))
            .insert_header(("X-Filename", "final.jpg"))
            .cookie(cookie.clone())
            .set_payload(body.to_vec())
            .to_request();
        let (status, photo) = call_request(app, request).await;

        assert_eq!(status, StatusCode::CREATED);

        photo["id"].as_str().unwrap().to_owned()
    }

    async fn list_set<S>(
        app: &S,
        cookie: &Cookie<'static>,
        gallery_id: &str,
        finals: bool,
    ) -> Vec<Value>
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/v1/galleries/{gallery_id}/photos{}",
                if finals { "?finals=true" } else { "" }
            ))
            .cookie(cookie.clone());
        let (status, body) = call(app, request).await;

        assert_eq!(status, StatusCode::OK);

        body.as_array().unwrap().clone()
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn finals_are_uploaded_unchanged_and_listed_apart_from_proofs(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let proof = waiting_photo(&app, &cookie, &gallery_id).await;
        let bytes = jpeg(3, 2);
        let finished = upload_final(&app, &cookie, &gallery_id, &bytes).await;

        let finals = list_set(&app, &cookie, &gallery_id, true).await;
        let proofs = list_set(&app, &cookie, &gallery_id, false).await;
        let stored = photo_files(directory.path(), &finished);

        assert_eq!(finals.len(), 1);
        assert_eq!(finals[0]["id"], finished);
        assert_eq!(finals[0]["status"], "waiting");
        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0]["id"], proof);
        assert_eq!(stored.len(), 1);
        assert_eq!(fs::read(&stored[0]).unwrap(), bytes);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn finals_reject_non_jpeg_and_malformed_files_without_effects(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool.clone(), directory.path(), upload_config(1024, 100)).await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;

        for (content_type, body, expected) in [
            (
                "text/plain",
                b"x".to_vec(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ),
            (
                "image/jpeg",
                b"not a jpeg".to_vec(),
                StatusCode::UNPROCESSABLE_ENTITY,
            ),
        ] {
            let request = test::TestRequest::post()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/photos?finals=true"
                ))
                .insert_header(("Origin", ORIGIN))
                .insert_header(("Content-Type", content_type))
                .insert_header(("X-Filename", "final.jpg"))
                .cookie(cookie.clone())
                .set_payload(body)
                .to_request();
            let (status, _) = call_request(&app, request).await;

            assert_eq!(status, expected);
        }

        assert_no_upload_effects(&pool, directory.path()).await;
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn an_unreadable_finals_flag_is_a_bad_request(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let request = test::TestRequest::get()
            .uri(&format!(
                "/api/v1/galleries/{gallery_id}/photos?finals=maybe"
            ))
            .cookie(cookie.clone());
        let (status, _) = call(&app, request).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn finals_are_only_served_ready_and_only_when_asked_for(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let finished = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;
        let get = |query: &str| {
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/photos/{finished}/thumbnail{query}"
                ))
                .cookie(cookie.clone())
                .to_request()
        };

        assert_eq!(
            test::call_service(&app, get("?finals=true")).await.status(),
            StatusCode::NOT_FOUND
        );

        Database::new(pool.clone())
            .repository::<TestRepository>()
            .force_photo_status(photo_id(&finished), &PhotoStatus::Ready)
            .await
            .unwrap();
        let stored = store_derivative(&pool, directory.path(), &finished, "thumbnail").await;
        let served = test::call_service(&app, get("?finals=true")).await;

        assert_eq!(served.status(), StatusCode::OK);
        assert_eq!(test::read_body(served).await, stored);
        assert_eq!(
            test::call_service(&app, get("")).await.status(),
            StatusCode::NOT_FOUND
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn clients_never_see_finals(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let finished = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;

        Database::new(pool.clone())
            .repository::<TestRepository>()
            .force_photo_status(photo_id(&finished), &PhotoStatus::Ready)
            .await
            .unwrap();
        store_derivative(&pool, directory.path(), &finished, "thumbnail").await;

        let share = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/share"))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone());
        let (_, gallery) = call(&app, share).await;
        let token = gallery["share_token"].as_str().unwrap();
        let (_, listed) = call(
            &app,
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}/photos")),
        )
        .await;
        let media = test::call_service(
            &app,
            test::TestRequest::get()
                .uri(&format!(
                    "/api/v1/galleries/shared/{token}/photos/{finished}/thumbnail"
                ))
                .to_request(),
        )
        .await;
        let favorite = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!(
                    "/api/v1/galleries/shared/{token}/favorites/{finished}"
                ))
                .insert_header(("Origin", ORIGIN))
                .to_request(),
        )
        .await;

        assert_eq!(listed, json!([]));
        assert_eq!(media.status(), StatusCode::NOT_FOUND);
        assert_eq!(favorite.status(), StatusCode::NOT_FOUND);
    }

    fn gallery_post(
        gallery_id: &str,
        action: &str,
        cookie: Option<&Cookie<'static>>,
    ) -> test::TestRequest {
        let request = test::TestRequest::post()
            .uri(&format!("/api/v1/galleries/{gallery_id}/{action}"))
            .insert_header(("Origin", ORIGIN));

        match cookie {
            Some(cookie) => request.cookie(cookie.clone()),
            None => request,
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn deliver_needs_a_session_and_a_shared_gallery_and_switches_the_link(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let foreign = Database::new(pool.clone())
            .repository::<GalleryRepository>()
            .create(other_workspace(&pool).await)
            .await
            .unwrap()
            .id
            .0
            .to_string();

        let (status, _) = call(&app, gallery_post(&gallery_id, "deliver", None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);

        let (status, _) = call(&app, gallery_post(&foreign, "deliver", Some(&cookie))).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        let (status, _) = call(&app, gallery_post(&gallery_id, "deliver", Some(&cookie))).await;
        assert_eq!(status, StatusCode::CONFLICT);

        let (_, shared) = call(&app, gallery_post(&gallery_id, "share", Some(&cookie))).await;
        let token = shared["share_token"].as_str().unwrap().to_owned();
        let (status, delivered) =
            call(&app, gallery_post(&gallery_id, "deliver", Some(&cookie))).await;
        let (again, _) = call(&app, gallery_post(&gallery_id, "deliver", Some(&cookie))).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(again, StatusCode::OK);
        assert_eq!(delivered["states"], json!(["delivered"]));

        let (_, page) = call(
            &app,
            test::TestRequest::get().uri(&format!("/api/v1/galleries/shared/{token}")),
        )
        .await;

        assert_eq!(page["delivered"], true);
    }

    async fn client_get<S>(app: &S, token: &str, path: &str) -> (StatusCode, Value)
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        call_request(
            app,
            test::TestRequest::get()
                .uri(&format!("/api/v1/galleries/shared/{token}{path}"))
                .to_request(),
        )
        .await
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_delivered_link_serves_only_ready_finals_and_follows_their_removal(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let proof = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let ready = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;
        let waiting = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;
        let database = Database::new(pool.clone());

        database
            .repository::<TestRepository>()
            .force_photo_status(photo_id(&ready), &PhotoStatus::Ready)
            .await
            .unwrap();
        store_derivative(&pool, directory.path(), &proof, "thumbnail").await;
        store_derivative(&pool, directory.path(), &ready, "thumbnail").await;
        store_derivative(&pool, directory.path(), &waiting, "thumbnail").await;

        let (_, shared) = call(&app, gallery_post(&gallery_id, "share", Some(&cookie))).await;
        let token = shared["share_token"].as_str().unwrap().to_owned();
        let listed = |body: Value| -> Vec<String> {
            body.as_array()
                .unwrap()
                .iter()
                .map(|photo| photo["id"].as_str().unwrap().to_owned())
                .collect()
        };

        assert_eq!(
            listed(client_get(&app, &token, "/photos").await.1),
            [proof.as_str()]
        );

        call(&app, gallery_post(&gallery_id, "deliver", Some(&cookie))).await;

        assert_eq!(
            listed(client_get(&app, &token, "/photos").await.1),
            [ready.as_str()]
        );
        assert_eq!(
            client_get(&app, &token, &format!("/photos/{ready}/thumbnail"))
                .await
                .0,
            StatusCode::OK
        );

        for hidden in [&proof, &waiting] {
            assert_eq!(
                client_get(&app, &token, &format!("/photos/{hidden}/thumbnail"))
                    .await
                    .0,
                StatusCode::NOT_FOUND
            );
        }

        let favorite = test::call_service(
            &app,
            test::TestRequest::put()
                .uri(&format!(
                    "/api/v1/galleries/shared/{token}/favorites/{ready}"
                ))
                .insert_header(("Origin", ORIGIN))
                .to_request(),
        )
        .await;

        assert_eq!(favorite.status(), StatusCode::CONFLICT);

        let removed = test::call_service(
            &app,
            delete_request(
                Some(&cookie),
                &format!("{gallery_id}/photos/{ready}?finals=true"),
            ),
        )
        .await;

        assert!(removed.status().is_success());
        assert_eq!(
            listed(client_get(&app, &token, "/photos").await.1),
            Vec::<String>::new()
        );
        assert_eq!(
            client_get(&app, &token, &format!("/photos/{ready}/thumbnail"))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(client_get(&app, &token, "").await.0, StatusCode::OK);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn finals_are_removed_singly_and_in_bulk_and_failed_ones_retried(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let proof = waiting_photo(&app, &cookie, &gallery_id).await;
        let first = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;
        let second = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;
        let third = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;

        Database::new(pool.clone())
            .repository::<TestRepository>()
            .force_photo_status(photo_id(&third), &PhotoStatus::Failed("broken".to_owned()))
            .await
            .unwrap();

        let retry = test::TestRequest::post()
            .uri(&format!(
                "/api/v1/galleries/{gallery_id}/photos/retry?finals=true"
            ))
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone());
        let (status, body) = call(&app, retry).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["requeued"], 1);
        assert_eq!(
            test::call_service(
                &app,
                delete_request(
                    Some(&cookie),
                    &format!("{gallery_id}/photos/{proof}?finals=true")
                )
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            test::call_service(
                &app,
                delete_request(
                    Some(&cookie),
                    &format!("{gallery_id}/photos/{first}?finals=true")
                )
            )
            .await
            .status(),
            StatusCode::NO_CONTENT
        );

        let bulk = test::TestRequest::post()
            .uri(&format!(
                "/api/v1/galleries/{gallery_id}/photos/delete?finals=true"
            ))
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", "application/json"))
            .cookie(cookie.clone())
            .set_payload(photo_ids(&[&second, &third, &proof]))
            .to_request();
        let (status, body) = call_request(&app, bulk).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["deleted"], 2);
        assert!(list_set(&app, &cookie, &gallery_id, true).await.is_empty());
        assert_eq!(list_set(&app, &cookie, &gallery_id, false).await.len(), 1);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn finals_routes_need_ownership(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let finished = upload_final(&app, &cookie, &gallery_id, &jpeg(3, 2)).await;

        for request in [
            test::TestRequest::get().uri(&format!(
                "/api/v1/galleries/{gallery_id}/photos?finals=true"
            )),
            test::TestRequest::post()
                .uri(&format!(
                    "/api/v1/galleries/{gallery_id}/photos/retry?finals=true"
                ))
                .insert_header(("Origin", ORIGIN)),
        ] {
            let response = test::call_service(&app, request.to_request()).await;

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }

        let response = test::call_service(
            &app,
            delete_request(None, &format!("{gallery_id}/photos/{finished}?finals=true")),
        )
        .await;

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    async fn client_bytes<S>(
        app: &S,
        request: test::TestRequest,
    ) -> (StatusCode, actix_web::http::header::HeaderMap, Vec<u8>)
    where
        S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
    {
        let response = test::call_service(app, request.to_request()).await;
        let status = response.status();
        let headers = response.headers().clone();

        (status, headers, test::read_body(response).await.to_vec())
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn a_client_downloads_delivered_finals_and_each_download_is_recorded_once(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(
            pool.clone(),
            directory.path(),
            upload_config(1024 * 1024, 1_000_000),
        )
        .await;
        let cookie = sign_up(&app).await;
        let gallery_id = create_gallery(&app, &cookie).await;
        let proof = photo_in(&app, &pool, &cookie, &gallery_id, Progress::Ready).await;
        let bytes = jpeg(3, 2);
        let ready = upload_final(&app, &cookie, &gallery_id, &bytes).await;
        let waiting = upload_final(&app, &cookie, &gallery_id, &bytes).await;

        Database::new(pool.clone())
            .repository::<TestRepository>()
            .force_photo_status(photo_id(&ready), &PhotoStatus::Ready)
            .await
            .unwrap();

        let (_, shared) = call(&app, gallery_post(&gallery_id, "share", Some(&cookie))).await;
        let token = shared["share_token"].as_str().unwrap().to_owned();
        let single = |photo: &str| {
            test::TestRequest::get().uri(&format!(
                "/api/v1/galleries/shared/{token}/photos/{photo}/download"
            ))
        };
        let zip = |post: bool| {
            let request = if post {
                test::TestRequest::post().insert_header(("Origin", ORIGIN))
            } else {
                test::TestRequest::get()
            };

            request.uri(&format!("/api/v1/galleries/shared/{token}/download"))
        };
        let downloaded = async |id: &str| -> bool {
            sqlx::query_scalar("SELECT downloaded_at IS NOT NULL FROM photos WHERE id = $1::uuid")
                .bind(id)
                .fetch_one(&pool)
                .await
                .unwrap()
        };

        assert_eq!(
            client_bytes(&app, single(&ready)).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(client_bytes(&app, zip(true)).await.0, StatusCode::NOT_FOUND);

        call(&app, gallery_post(&gallery_id, "deliver", Some(&cookie))).await;

        for hidden in [&proof, &waiting] {
            assert_eq!(
                client_bytes(&app, single(hidden)).await.0,
                StatusCode::NOT_FOUND
            );
        }

        sqlx::query(
            "UPDATE photos SET original_name = E'../dir\\\\a\\r\\nb.jpg' WHERE id = $1::uuid",
        )
        .bind(&ready)
        .execute(&pool)
        .await
        .unwrap();

        let (status, headers, body) = client_bytes(&app, single(&ready)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, bytes);
        assert_eq!(
            headers.get("content-disposition").unwrap(),
            "attachment; filename=\"ab.jpg\""
        );
        assert_eq!(headers.get("cache-control").unwrap(), "private, no-store");
        assert_eq!(headers.get("x-content-type-options").unwrap(), "nosniff");
        assert!(
            headers
                .get("content-disposition")
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("attachment")
        );
        assert!(downloaded(&ready).await);
        assert!(!downloaded(&waiting).await);

        let (status, body) = call_request(&app, zip(true).to_request()).await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(body["status"], "building");
        assert_eq!(client_bytes(&app, zip(false)).await.0, StatusCode::CONFLICT);

        let (workspace_id, export_id): (Uuid, Uuid) =
            sqlx::query_as("SELECT workspace_id, id FROM exports WHERE kind = 'delivery'")
                .fetch_one(&pool)
                .await
                .unwrap();
        let location = pp_database::photo::PhotoLocation {
            workspace_id: pp_database::auth::WorkspaceId(workspace_id),
            gallery_id: GalleryId(Uuid::parse_str(&gallery_id).unwrap()),
        };
        let storage = pp_storage::PhotoStorage::open(
            &pp_core::config::PhotoDir::try_from(directory.path().to_owned()).unwrap(),
        )
        .await
        .unwrap();

        storage
            .build_archive(
                &pp_storage::ArchiveKey {
                    workspace: location.workspace_id,
                    gallery: location.gallery_id,
                    export: pp_database::export::ExportId(export_id),
                },
                vec![pp_storage::ArchiveEntry {
                    source: pp_storage::PhotoKey::with_location(
                        location,
                        photo_id(&ready),
                        pp_storage::Variant::Original,
                    ),
                    original_name: "final.jpg".to_owned(),
                }],
            )
            .await
            .unwrap();
        sqlx::query("UPDATE exports SET status = '{\"kind\": \"ready\"}', finished_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        let (status, body) = call_request(&app, zip(true).to_request()).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ready");

        let (status, headers, body) = client_bytes(&app, zip(false)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers.get("content-type").unwrap(), "application/zip");
        assert!(body.starts_with(b"PK"));

        let recorded: Option<time::OffsetDateTime> =
            sqlx::query_scalar("SELECT downloaded_at FROM exports WHERE kind = 'delivery'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert!(recorded.is_some());

        client_bytes(&app, zip(false)).await;

        let again: Option<time::OffsetDateTime> =
            sqlx::query_scalar("SELECT downloaded_at FROM exports WHERE kind = 'delivery'")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(again, recorded);

        sqlx::query("UPDATE galleries SET archived_at = now()")
            .execute(&pool)
            .await
            .unwrap();

        assert_eq!(
            client_bytes(&app, zip(false)).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            client_bytes(&app, single(&ready)).await.0,
            StatusCode::NOT_FOUND
        );
    }
}
