//! Bounded upload resources and the staged-photo validation and publication pipeline.

use std::{
    future::{Ready, ready},
    num::{NonZeroU32, NonZeroU64},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use futures_util::StreamExt;
use libvips::{VipsApp, VipsImage};
use pp_database::{photo::PhotoDimensions, photo_intake::PhotoIntakeError};
use pp_storage::{PhotoKey, PhotoStorage, StagedPhoto, StorageError};
use thiserror::Error;
use tokio::{fs::File, io::AsyncReadExt, sync::OwnedSemaphorePermit};

use crate::{api::ApiState, error::ErrorResponse};

const MAX_FILENAME_BYTES: usize = 255;
const UPLOAD_BODY_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

static VIPS_APP: OnceLock<Mutex<Option<VipsApp>>> = OnceLock::new();

/// Initializes process-wide image inspection with bounded concurrency and no operation cache.
pub fn install() -> Result<(), PhotoUploadError> {
    let app = VIPS_APP.get_or_init(|| Mutex::new(None));
    let mut app = app
        .lock()
        .map_err(|_| PhotoUploadError::InitializationLock)?;

    if app.is_none() {
        let initialized = VipsApp::new("photo-proofing-api", false)
            .map_err(|error| PhotoUploadError::ImageLibrary(format!("{error:?}")))?;

        // Header inspection does not need a parallel pixel pipeline or cached operations.
        initialized.concurrency_set(1);
        initialized.cache_set_max(0);
        initialized.cache_set_max_mem(0);
        initialized.cache_set_max_files(0);
        *app = Some(initialized);
    }

    Ok(())
}

/// Failure to initialize bounded upload metadata support.
#[derive(Debug, Error)]
pub enum PhotoUploadError {
    #[error("image library initialization failed: {0}")]
    ImageLibrary(String),
    #[error("image library initialization state is poisoned")]
    InitializationLock,
}

/// Upload admission held until the handler finishes, fails validation, or is cancelled.
///
/// Extraction fails fast with 503 before handler validation when all slots are occupied.
/// The extractor never consumes the request body; staging remains authorization-gated.
#[must_use]
pub(crate) struct UploadSlot {
    state: Data<ApiState>,
    _permit: OwnedSemaphorePermit,
}

impl FromRequest for UploadSlot {
    type Error = ErrorResponse;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        let Some(state) = request.app_data::<Data<ApiState>>().cloned() else {
            tracing::error!("upload request is missing API application state");

            return ready(Err(ErrorResponse::unavailable()));
        };
        let Some(permit) = state.try_acquire_upload_slot() else {
            return ready(Err(ErrorResponse::unavailable()));
        };

        ready(Ok(Self {
            state,
            _permit: permit,
        }))
    }
}

impl UploadSlot {
    pub(crate) async fn stage_payload(
        &self,
        payload: Payload,
    ) -> Result<(StagedPhoto, NonZeroU64), ErrorResponse> {
        self.stage_payload_with_idle_timeout(payload, UPLOAD_BODY_IDLE_TIMEOUT)
            .await
    }

    async fn stage_payload_with_idle_timeout(
        &self,
        mut payload: Payload,
        idle_timeout: Duration,
    ) -> Result<(StagedPhoto, NonZeroU64), ErrorResponse> {
        let mut staged = self
            .state
            .storage()
            .stage()
            .await
            .map_err(storage_failure)?;
        let mut size = 0_u64;

        loop {
            let Some(chunk) = tokio::time::timeout(idle_timeout, payload.next())
                .await
                .map_err(|_| ErrorResponse::request_timeout())?
            else {
                break;
            };
            let chunk = chunk.map_err(|_| ErrorResponse::bad_request())?;
            let chunk_size =
                u64::try_from(chunk.len()).map_err(|_| ErrorResponse::payload_too_large())?;
            size = size
                .checked_add(chunk_size)
                .ok_or_else(ErrorResponse::payload_too_large)?;

            if size > self.state.upload_config().max_size.get() {
                return Err(ErrorResponse::payload_too_large());
            }

            staged.write(&chunk).await.map_err(storage_failure)?;
        }

        let size = NonZeroU64::new(size).ok_or_else(ErrorResponse::invalid_image)?;

        staged.flush().await.map_err(storage_failure)?;

        Ok((staged, size))
    }
}

/// API upload policy for a storage-owned staged file, without coupling storage to HTTP or libvips.
pub(crate) trait StagedPhotoExt {
    /// Reads JPEG metadata without decoding pixels and rejects dimensions above `max_pixels`.
    async fn inspect(&self, max_pixels: NonZeroU64) -> Result<PhotoDimensions, ErrorResponse>;

    /// Publishes an original, cleaning up partial publication but never a colliding original.
    async fn commit_original(
        self,
        storage: &PhotoStorage,
        key: &PhotoKey,
    ) -> Result<(), ErrorResponse>;
}

impl StagedPhotoExt for StagedPhoto {
    async fn inspect(&self, max_pixels: NonZeroU64) -> Result<PhotoDimensions, ErrorResponse> {
        let mut file = File::open(self.temporary_path())
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

        let path = self
            .temporary_path()
            .to_str()
            .ok_or_else(ErrorResponse::unavailable)?
            .to_owned();
        let path = format!("{path}[fail-on=truncated]");
        let dimensions = tokio::task::spawn_blocking(move || {
            // The generic loader accepts this without version-specific JPEG options.
            let image = VipsImage::new_from_file(&path).map_err(|_| ())?;
            let width =
                NonZeroU32::new(u32::try_from(image.get_width()).map_err(|_| ())?).ok_or(())?;
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

    async fn commit_original(
        self,
        storage: &PhotoStorage,
        key: &PhotoKey,
    ) -> Result<(), ErrorResponse> {
        match self.commit(key).await {
            Ok(()) => Ok(()),
            Err(error @ StorageError::AlreadyExists) => Err(storage_failure(error)),
            Err(error) => {
                let response = storage_failure(error);

                remove_original(storage, key).await?;

                Err(response)
            }
        }
    }
}

fn validate_pixel_count(
    width: u64,
    height: u64,
    max_pixels: NonZeroU64,
) -> Result<(), ErrorResponse> {
    let pixels = width
        .checked_mul(height)
        .ok_or_else(ErrorResponse::dimensions_too_large)?;

    if pixels > max_pixels.get() {
        return Err(ErrorResponse::dimensions_too_large());
    }

    Ok(())
}

pub(crate) fn original_name(request: &HttpRequest) -> String {
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

pub(crate) async fn remove_original(
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

pub(crate) async fn cleanup(
    storage: &PhotoStorage,
    key: &PhotoKey,
    error: &PhotoIntakeError,
) -> Result<(), ErrorResponse> {
    match error {
        PhotoIntakeError::Database(_) => {
            tracing::error!(%error, photo_id = %key.photo, "photo metadata transaction failed");
            remove_original(storage, key).await
        }
        PhotoIntakeError::CommitOutcomeUnknown { .. } => {
            tracing::error!(
                %error,
                photo_id = %key.photo,
                "photo commit outcome is unknown; preserving published original"
            );

            Ok(())
        }
    }
}

pub(crate) fn storage_failure(error: StorageError) -> ErrorResponse {
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
        time::Duration,
    };

    use actix_web::{ResponseError, http::StatusCode};
    use libvips::ops::{black, jpegsave_buffer};
    use pp_core::config::PhotoDir;
    use pp_core::notifications::PhotoId;
    use pp_database::{
        auth::WorkspaceId, error::DatabaseError, gallery::GalleryId, photo_intake::PhotoIntakeError,
    };
    use pp_storage::{PhotoKey, PhotoStorage, Variant};
    use sqlx::postgres::PgPoolOptions;
    use tokio::io::AsyncReadExt;
    use uuid::Uuid;

    use super::*;
    use crate::{config::UploadConfig, tests::state_with_upload};

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

    async fn upload_state(directory: &Path, concurrency: usize) -> ApiState {
        let photo_dir: PhotoDir = directory.to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();

        install().unwrap();

        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://localhost/unused")
            .unwrap();

        state_with_upload(
            pool,
            storage,
            UploadConfig {
                max_size: NonZeroU64::new(1024).unwrap(),
                max_pixels: NonZeroU64::new(2048).unwrap(),
                concurrency: NonZeroUsize::new(concurrency).unwrap(),
            },
        )
    }

    async fn upload_slot(state: ApiState) -> UploadSlot {
        let request = actix_web::test::TestRequest::default()
            .app_data(Data::new(state))
            .to_http_request();

        UploadSlot::extract(&request).await.unwrap()
    }

    fn photo_key() -> PhotoKey {
        PhotoKey {
            workspace: WorkspaceId(Uuid::from_u128(1)),
            gallery: GalleryId(Uuid::from_u128(2)),
            photo: PhotoId::from(Uuid::from_u128(3)),
            variant: Variant::Original,
        }
    }

    #[tokio::test]
    async fn concurrent_upload_slots_fail_fast_at_the_configured_limit() {
        let directory = tempfile::tempdir().unwrap();
        let state = upload_state(directory.path(), 4).await;
        let mut permits = (0..4)
            .map(|_| state.try_acquire_upload_slot().unwrap())
            .collect::<Vec<_>>();

        assert!(state.try_acquire_upload_slot().is_none());
        drop(permits.pop());
        assert!(state.try_acquire_upload_slot().is_some());
    }

    #[tokio::test]
    async fn interrupted_body_discards_staged_file() {
        let directory = tempfile::tempdir().unwrap();
        let state = upload_state(directory.path(), 1).await;
        let slot = upload_slot(state).await;
        let (mut sender, body) = actix_http::h1::Payload::create(false);

        sender.feed_data(actix_web::web::Bytes::from_static(b"partial jpeg"));
        sender.set_error(actix_http::error::PayloadError::Incomplete(None));

        let result = slot
            .stage_payload_with_idle_timeout(
                actix_web::dev::Payload::from(body),
                super::UPLOAD_BODY_IDLE_TIMEOUT,
            )
            .await;

        assert!(result.is_err());
        assert!(stored_files(directory.path()).is_empty());
    }

    #[tokio::test]
    async fn idle_body_timeout_discards_stage_and_releases_upload_slot() {
        let directory = tempfile::tempdir().unwrap();
        let state = upload_state(directory.path(), 1).await;
        let slot = upload_slot(state).await;
        let state = slot.state.clone();
        let (_sender, body) = actix_http::h1::Payload::create(false);

        let result = {
            let result = slot
                .stage_payload_with_idle_timeout(
                    actix_web::dev::Payload::from(body),
                    Duration::from_millis(10),
                )
                .await;

            drop(slot);

            result
        };
        let error = result.unwrap_err();

        assert_eq!(error.status_code(), StatusCode::REQUEST_TIMEOUT);
        assert!(stored_files(directory.path()).is_empty());
        assert!(state.try_acquire_upload_slot().is_some());
    }

    #[tokio::test]
    async fn full_storage_maps_to_service_unavailable() {
        let response = storage_failure(pp_storage::StorageError::DiskFull(std::io::Error::from(
            std::io::ErrorKind::StorageFull,
        )))
        .error_response();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn publish_collision_does_not_remove_the_existing_original() {
        let directory = tempfile::tempdir().unwrap();
        let state = upload_state(directory.path(), 1).await;
        let key = photo_key();
        let mut original = state.storage().stage().await.unwrap();

        original.write(b"existing original").await.unwrap();
        original.commit(&key).await.unwrap();

        let mut duplicate = state.storage().stage().await.unwrap();

        duplicate.write(b"new upload").await.unwrap();

        let error = duplicate
            .commit_original(state.storage(), &key)
            .await
            .unwrap_err();

        assert_eq!(error.status_code(), StatusCode::SERVICE_UNAVAILABLE);

        let mut stored = state.storage().read(&key).await.unwrap();
        let mut bytes = Vec::new();

        stored.read_to_end(&mut bytes).await.unwrap();

        assert_eq!(bytes, b"existing original");
        assert_eq!(stored_files(directory.path()).len(), 1);
    }

    #[tokio::test]
    async fn uncertain_database_commit_preserves_published_original() {
        let directory = tempfile::tempdir().unwrap();
        let state = upload_state(directory.path(), 1).await;
        let key = photo_key();
        let mut original = state.storage().stage().await.unwrap();

        original
            .write(b"possibly committed original")
            .await
            .unwrap();
        original.commit(&key).await.unwrap();

        let error = PhotoIntakeError::CommitOutcomeUnknown {
            commit_error: DatabaseError::Unreachable(std::io::Error::from(
                std::io::ErrorKind::ConnectionReset,
            )),
            reconciliation_error: None,
        };

        cleanup(state.storage(), &key, &error).await.unwrap();

        let mut stored = state.storage().read(&key).await.unwrap();
        let mut bytes = Vec::new();

        stored.read_to_end(&mut bytes).await.unwrap();

        assert_eq!(bytes, b"possibly committed original");
        assert_eq!(stored_files(directory.path()).len(), 1);
    }

    #[tokio::test]
    async fn overflowing_pixel_count_is_rejected() {
        assert!(validate_pixel_count(u64::MAX, 2, NonZeroU64::new(2048).unwrap()).is_err());
    }

    #[tokio::test]
    async fn staged_photo_inspects_and_publishes_unchanged_original() {
        let directory = tempfile::tempdir().unwrap();
        let state = upload_state(directory.path(), 1).await;
        let jpeg = jpegsave_buffer(&black(20, 10).unwrap()).unwrap();
        let mut staged = state.storage().stage().await.unwrap();

        staged.write(&jpeg).await.unwrap();
        staged.flush().await.unwrap();

        let dimensions = staged
            .inspect(state.upload_config().max_pixels)
            .await
            .unwrap();

        assert_eq!(dimensions.width.get(), 20);
        assert_eq!(dimensions.height.get(), 10);
        assert!(staged.inspect(NonZeroU64::new(199).unwrap()).await.is_err());

        let key = photo_key();

        staged.commit_original(state.storage(), &key).await.unwrap();

        let mut stored = state.storage().read(&key).await.unwrap();
        let mut bytes = Vec::new();

        stored.read_to_end(&mut bytes).await.unwrap();

        assert_eq!(bytes, jpeg);
        assert_eq!(stored_files(directory.path()).len(), 1);
    }
}
