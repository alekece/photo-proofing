//! Streaming JPEG uploads, validated without decoding pixels and registered with a durable job.
//!
//! The handler checks workspace access and request limits before consuming the body, stages bytes
//! in storage, inspects only JPEG metadata, then publishes the unchanged original and atomically
//! records its photo/job pair. Dropping a failed stage or removing a failed publication prevents
//! rejected uploads from becoming visible.

use std::num::{NonZeroU32, NonZeroU64};

use actix_web::{
    HttpRequest, HttpResponse,
    http::header::{CONTENT_LENGTH, CONTENT_TYPE},
    post,
    web::{self, Data, Path as WebPath, ServiceConfig},
};
use futures_util::StreamExt;
use libvips::VipsImage;
use pp_core::notifications::PhotoId;
use pp_database::{
    gallery::GalleryId,
    photo::{NewPhoto, PhotoDimensions, PhotoSize},
};
use pp_storage::{PhotoKey, PhotoStorage, StagedPhoto, StorageError, Variant};
use tokio::{fs::File, io::AsyncReadExt};
use uuid::Uuid;

use crate::{
    api::ApiState,
    config::{PixelLimit, UploadSize},
    error::ErrorResponse,
    extractors::identity::Authenticated,
    photo_upload::PhotoUploadState,
    response::ResponseBuilderExt,
};

const MAX_FILENAME_BYTES: usize = 255;

/// Registers photo routes inside the `/api/v1/galleries` scope.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(upload_photo);
}

#[post("/{id}/photos")]
async fn upload_photo(
    state: Data<ApiState>,
    upload: Data<PhotoUploadState>,
    Authenticated(identity): Authenticated,
    request: HttpRequest,
    id: WebPath<GalleryId>,
    payload: web::Payload,
) -> Result<HttpResponse, ErrorResponse> {
    let gallery_id = id.into_inner();

    if state
        .galleries()
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

    let max_size = upload.max_size().bytes().get();

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

    let _permit = upload
        .try_acquire_upload_slot()
        .ok_or_else(ErrorResponse::unavailable)?;
    let (staged, size) =
        stage_payload(upload.storage(), payload.into_inner(), upload.max_size()).await?;
    let dimensions = inspect_jpeg(&staged, upload.max_pixels()).await?;
    let photo_id = PhotoId::from(Uuid::new_v4());
    let key = PhotoKey {
        workspace: identity.workspace_id,
        gallery: gallery_id,
        photo: photo_id,
        variant: Variant::Original,
    };

    commit_original(upload.storage(), staged, &key).await?;

    let new_photo = NewPhoto {
        id: photo_id,
        workspace_id: identity.workspace_id,
        gallery_id,
        original_name: original_name(&request),
        size: PhotoSize::new(size),
        dimensions,
    };

    let photo = match state.photos().create_with_job(new_photo).await {
        Ok(Some(photo)) => photo,
        Ok(None) => {
            remove_published_original(upload.storage(), &key).await?;

            return Err(ErrorResponse::not_found());
        }
        Err(error) => {
            tracing::error!(%error, "photo metadata transaction failed");
            remove_published_original(upload.storage(), &key).await?;

            return Err(ErrorResponse::unavailable());
        }
    };

    Ok(HttpResponse::Created().no_store().json(photo))
}

async fn stage_payload(
    storage: &PhotoStorage,
    mut payload: actix_web::dev::Payload,
    limit: UploadSize,
) -> Result<(StagedPhoto, NonZeroU64), ErrorResponse> {
    let mut staged = storage.stage().await.map_err(storage_failure)?;
    let mut size = 0_u64;

    while let Some(chunk) = payload.next().await {
        let chunk = chunk.map_err(|_| ErrorResponse::bad_request())?;
        let chunk_size =
            u64::try_from(chunk.len()).map_err(|_| ErrorResponse::payload_too_large())?;
        size = size
            .checked_add(chunk_size)
            .ok_or_else(ErrorResponse::payload_too_large)?;

        if size > limit.bytes().get() {
            return Err(ErrorResponse::payload_too_large());
        }

        staged.write(&chunk).await.map_err(storage_failure)?;
    }

    let size = NonZeroU64::new(size).ok_or_else(ErrorResponse::invalid_image)?;

    staged.flush().await.map_err(storage_failure)?;

    Ok((staged, size))
}

async fn inspect_jpeg(
    staged: &StagedPhoto,
    max_pixels: PixelLimit,
) -> Result<PhotoDimensions, ErrorResponse> {
    let mut file = File::open(staged.temporary_path())
        .await
        .map_err(|error| storage_failure(error.into()))?;
    let mut signature = [0; 3];

    if let Err(error) = file.read_exact(&mut signature).await {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            return Err(ErrorResponse::invalid_image());
        }

        return Err(storage_failure(error.into()));
    }

    if signature != [0xff, 0xd8, 0xff] {
        return Err(ErrorResponse::invalid_image());
    }

    let path = staged
        .temporary_path()
        .to_str()
        .ok_or_else(ErrorResponse::unavailable)?
        .to_owned();
    let dimensions = tokio::task::spawn_blocking(move || {
        let image = VipsImage::new_from_file(&path).map_err(|_| ())?;
        let width = NonZeroU32::new(u32::try_from(image.get_width()).map_err(|_| ())?).ok_or(())?;
        let height =
            NonZeroU32::new(u32::try_from(image.get_height()).map_err(|_| ())?).ok_or(())?;

        Ok::<_, ()>(PhotoDimensions { width, height })
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "JPEG metadata inspection task failed");
        ErrorResponse::unavailable()
    })?
    .map_err(|_| ErrorResponse::invalid_image())?;
    validate_pixel_count(
        u64::from(dimensions.width.get()),
        u64::from(dimensions.height.get()),
        max_pixels,
    )?;

    Ok(dimensions)
}

fn validate_pixel_count(
    width: u64,
    height: u64,
    max_pixels: PixelLimit,
) -> Result<(), ErrorResponse> {
    let pixels = width
        .checked_mul(height)
        .ok_or_else(ErrorResponse::dimensions_too_large)?;

    if pixels > max_pixels.pixels().get() {
        return Err(ErrorResponse::dimensions_too_large());
    }

    Ok(())
}

fn original_name(request: &HttpRequest) -> String {
    request
        .headers()
        .get("X-Filename")
        .and_then(|value| value.to_str().ok())
        .and_then(decode_filename)
        .filter(|value| !value.contains('\0') && value.len() <= MAX_FILENAME_BYTES)
        .unwrap_or_else(|| "photo.jpg".to_owned())
}

fn decode_filename(value: &str) -> Option<String> {
    let mut decoded = Vec::with_capacity(value.len());
    let mut bytes = value.bytes();

    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = hex_digit(bytes.next()?)?;
            let low = hex_digit(bytes.next()?)?;

            decoded.push((high << 4) | low);
        } else {
            decoded.push(byte);
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

async fn commit_original(
    storage: &PhotoStorage,
    staged: StagedPhoto,
    key: &PhotoKey,
) -> Result<(), ErrorResponse> {
    match staged.commit(key).await {
        Ok(()) => Ok(()),
        Err(error @ StorageError::AlreadyExists) => Err(storage_failure(error)),
        Err(error) => {
            let response = storage_failure(error);

            remove_published_original(storage, key).await?;

            Err(response)
        }
    }
}

async fn remove_published_original(
    storage: &PhotoStorage,
    key: &PhotoKey,
) -> Result<(), ErrorResponse> {
    match storage.remove(key).await {
        Ok(()) | Err(StorageError::NotFound) => Ok(()),
        Err(error) => {
            tracing::error!(%error, "failed to remove unpublished photo original");

            Err(ErrorResponse::unavailable())
        }
    }
}

fn storage_failure(error: StorageError) -> ErrorResponse {
    match error {
        StorageError::DiskFull(_) => ErrorResponse::storage_full(),
        error => {
            tracing::error!(%error, "photo storage operation failed");

            ErrorResponse::unavailable()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        num::{NonZeroU64, NonZeroUsize},
        path::{Path, PathBuf},
    };

    use actix_web::{
        App, ResponseError,
        cookie::Cookie,
        dev::{Service, ServiceResponse},
        http::{StatusCode, header::CONTENT_LENGTH},
        test,
        web::Data,
    };
    use libvips::ops::{black, jpegsave_buffer};
    use pp_core::config::PhotoDir;
    use pp_core::notifications::PhotoId;
    use pp_database::{
        Database,
        auth::WorkspaceId,
        gallery::{GalleryId, GalleryRepository},
    };
    use pp_storage::{PhotoKey, PhotoStorage, Variant};
    use serde_json::Value;
    use sqlx::PgPool;
    use tokio::io::AsyncReadExt;
    use uuid::Uuid;

    use crate::{
        api::configure_routes,
        config::{PhotoUploadConfig, PixelLimit, UploadSize},
        photo_upload::{PhotoUploadState, initialize_image_library},
        tests::{ORIGIN, app_state, call, other_workspace, sign_up},
    };

    fn upload_config(max_size: u64, max_pixels: u64) -> PhotoUploadConfig {
        PhotoUploadConfig {
            max_size: UploadSize::new(NonZeroU64::new(max_size).unwrap()),
            max_pixels: PixelLimit::new(NonZeroU64::new(max_pixels).unwrap()),
            concurrency: NonZeroUsize::new(4).unwrap(),
        }
    }

    async fn app(
        pool: PgPool,
        photo_dir: &Path,
        config: PhotoUploadConfig,
    ) -> impl Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>
    {
        initialize_image_library().unwrap();

        let photo_dir: PhotoDir = photo_dir.to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();
        let photo_upload = PhotoUploadState::new(storage, config);

        test::init_service(
            App::new()
                .app_data(Data::new(app_state(pool)))
                .app_data(Data::new(photo_upload))
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

    #[tokio::test]
    async fn interrupted_body_discards_staged_file() {
        initialize_image_library().unwrap();

        let directory = tempfile::tempdir().unwrap();
        let photo_dir: PhotoDir = directory.path().to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();
        let (mut sender, body) = actix_http::h1::Payload::create(false);

        sender.feed_data(actix_web::web::Bytes::from_static(b"partial jpeg"));
        sender.set_error(actix_http::error::PayloadError::Incomplete(None));

        let result = super::stage_payload(
            &storage,
            actix_web::dev::Payload::from(body),
            UploadSize::new(NonZeroU64::new(1024).unwrap()),
        )
        .await;

        assert!(result.is_err());
        assert!(stored_files(directory.path()).is_empty());
    }

    #[tokio::test]
    async fn full_storage_maps_to_insufficient_storage() {
        let response = super::storage_failure(pp_storage::StorageError::DiskFull(
            std::io::Error::from(std::io::ErrorKind::StorageFull),
        ))
        .error_response();

        assert_eq!(response.status(), StatusCode::INSUFFICIENT_STORAGE);
    }

    #[tokio::test]
    async fn publish_collision_does_not_remove_the_existing_original() {
        let directory = tempfile::tempdir().unwrap();
        let photo_dir: PhotoDir = directory.path().to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();
        let key = PhotoKey {
            workspace: WorkspaceId(Uuid::from_u128(1)),
            gallery: GalleryId(Uuid::from_u128(2)),
            photo: PhotoId::from(Uuid::from_u128(3)),
            variant: Variant::Original,
        };
        let mut original = storage.stage().await.unwrap();

        original.write(b"existing original").await.unwrap();
        original.commit(&key).await.unwrap();

        let mut duplicate = storage.stage().await.unwrap();

        duplicate.write(b"new upload").await.unwrap();

        let error = super::commit_original(&storage, duplicate, &key)
            .await
            .unwrap_err();

        assert_eq!(error.status_code(), StatusCode::SERVICE_UNAVAILABLE);

        let mut stored = storage.read(&key).await.unwrap();
        let mut bytes = Vec::new();

        stored.read_to_end(&mut bytes).await.unwrap();

        assert_eq!(bytes, b"existing original");
        assert_eq!(stored_files(directory.path()).len(), 1);
    }

    #[tokio::test]
    async fn overflowing_pixel_count_is_rejected() {
        let result = super::validate_pixel_count(
            u64::MAX,
            2,
            PixelLimit::new(NonZeroU64::new(100).unwrap()),
        );

        assert!(result.is_err());
    }
}
