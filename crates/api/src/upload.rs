//! Receives an authorized photo upload into bounded, storage-owned staging.
//!
//! The explicit constructor checks headers before receiving bytes, inspects the staged JPEG
//! through [`jpeg`], and collects metadata. The route owns publication and database acceptance.

pub(crate) mod jpeg;
pub(crate) mod limiter;

use std::{num::NonZeroU64, time::Duration};

use actix_web::{
    HttpRequest,
    dev::Payload,
    http::header::{CONTENT_LENGTH, CONTENT_TYPE},
};
use futures_util::StreamExt;
use pp_database::photo::PhotoDimensions;
use pp_storage::{PhotoStorage, StagedPhoto};

use crate::{config::UploadConfig, error::ErrorResponse};

const MAX_FILENAME_BYTES: usize = 255;
const UPLOAD_BODY_IDLE_TIMEOUT: Duration = Duration::from_secs(30);

/// A received and inspected upload; nothing is published until storage commits its staged bytes.
#[must_use = "a photo upload is discarded unless stored"]
pub(crate) struct PhotoUpload {
    pub(crate) staged: StagedPhoto,
    pub(crate) original_name: String,
    pub(crate) size: NonZeroU64,
    pub(crate) dimensions: PhotoDimensions,
}

impl PhotoUpload {
    /// Checks headers, receives bounded bytes into storage staging and inspects the JPEG.
    ///
    /// Call explicitly after gallery authorization, while holding an upload slot. This is
    /// deliberately not an Actix extractor, since extraction precedes handler authorization.
    pub(crate) async fn from_request(
        request: &HttpRequest,
        payload: Payload,
        storage: &PhotoStorage,
        options: &UploadConfig,
    ) -> Result<Self, ErrorResponse> {
        if request
            .headers()
            .get(CONTENT_TYPE)
            .is_none_or(|value| value.as_bytes() != b"image/jpeg")
        {
            return Err(ErrorResponse::unsupported_media_type());
        }

        if let Some(length) = request.headers().get(CONTENT_LENGTH) {
            let length = length
                .to_str()
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(ErrorResponse::bad_request)?;

            if length > options.max_size.get() {
                return Err(ErrorResponse::payload_too_large());
            }
        }

        let (staged, size) =
            stage_payload(storage, payload, options.max_size, UPLOAD_BODY_IDLE_TIMEOUT).await?;
        let dimensions = jpeg::inspect(staged.temporary_path(), options.max_pixels).await?;

        Ok(Self {
            staged,
            original_name: original_name(request),
            size,
            dimensions,
        })
    }
}

async fn stage_payload(
    storage: &PhotoStorage,
    mut payload: Payload,
    max_size: NonZeroU64,
    idle_timeout: Duration,
) -> Result<(StagedPhoto, NonZeroU64), ErrorResponse> {
    let mut staged = storage.stage().await?;
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

        if size > max_size.get() {
            return Err(ErrorResponse::payload_too_large());
        }

        staged.write(&chunk).await?;
    }

    let size = NonZeroU64::new(size).ok_or_else(ErrorResponse::invalid_image)?;

    staged.flush().await?;

    Ok((staged, size))
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        num::{NonZeroU64, NonZeroUsize},
        path::{Path, PathBuf},
        time::Duration,
    };

    use actix_web::{FromRequest, ResponseError, http::StatusCode, web::Data};
    use libvips::ops::{black, jpegsave_buffer};
    use pp_core::config::PhotoDir;
    use pp_storage::PhotoStorage;

    use super::*;
    use crate::{
        config::UploadConfig,
        upload::limiter::{UploadLimiter, UploadSlot},
    };

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

    async fn upload_resources(directory: &Path) -> (PhotoStorage, UploadConfig) {
        let photo_dir: PhotoDir = directory.to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();

        jpeg::install().unwrap();

        (
            storage,
            UploadConfig {
                max_size: NonZeroU64::new(1024).unwrap(),
                max_pixels: NonZeroU64::new(2048).unwrap(),
                concurrency: NonZeroUsize::new(1).unwrap(),
            },
        )
    }

    #[tokio::test]
    async fn photo_upload_rejects_headers_before_staging_or_reading_the_body() {
        let directory = tempfile::tempdir().unwrap();
        let (storage, options) = upload_resources(directory.path()).await;

        fs::remove_dir(directory.path().join(".tmp")).unwrap();

        for (content_type, length, status) in [
            ("text/plain", "1", StatusCode::UNSUPPORTED_MEDIA_TYPE),
            ("image/jpeg", "invalid", StatusCode::BAD_REQUEST),
            ("image/jpeg", "1025", StatusCode::PAYLOAD_TOO_LARGE),
        ] {
            let request = actix_web::test::TestRequest::post()
                .insert_header(("Content-Type", content_type))
                .insert_header(("Content-Length", length))
                .to_http_request();
            let body = futures_util::stream::poll_fn(
                |_| -> std::task::Poll<
                    Option<Result<actix_web::web::Bytes, actix_http::error::PayloadError>>,
                > {
                    panic!("invalid headers must not consume the body");
                },
            );
            let body: actix_http::BoxedPayloadStream = Box::pin(body);
            let result = PhotoUpload::from_request(&request, body.into(), &storage, &options).await;

            assert_eq!(result.err().unwrap().status_code(), status);
            assert!(stored_files(directory.path()).is_empty());
        }
    }

    #[tokio::test]
    async fn photo_upload_returns_metadata_and_unchanged_staged_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let (storage, options) = upload_resources(directory.path()).await;
        let jpeg = jpegsave_buffer(&black(20, 10).unwrap()).unwrap();
        let request = actix_web::test::TestRequest::post()
            .insert_header(("Content-Type", "image/jpeg"))
            .to_http_request();
        let (mut sender, body) = actix_http::h1::Payload::create(false);

        sender.feed_data(actix_web::web::Bytes::from(jpeg.clone()));
        sender.feed_eof();

        let photo_upload = PhotoUpload::from_request(&request, body.into(), &storage, &options)
            .await
            .unwrap();

        assert_eq!(photo_upload.size.get(), u64::try_from(jpeg.len()).unwrap());
        assert_eq!(photo_upload.dimensions.width.get(), 20);
        assert_eq!(photo_upload.dimensions.height.get(), 10);
        assert_eq!(photo_upload.original_name, "photo.jpg");
        assert_eq!(
            fs::read(photo_upload.staged.temporary_path()).unwrap(),
            jpeg
        );
        drop(photo_upload);
        assert!(stored_files(directory.path()).is_empty());
    }

    #[tokio::test]
    async fn interrupted_body_discards_staged_file() {
        let directory = tempfile::tempdir().unwrap();
        let (storage, options) = upload_resources(directory.path()).await;
        let (mut sender, body) = actix_http::h1::Payload::create(false);

        sender.feed_data(actix_web::web::Bytes::from_static(b"partial jpeg"));
        sender.set_error(actix_http::error::PayloadError::Incomplete(None));

        let result = stage_payload(
            &storage,
            body.into(),
            options.max_size,
            UPLOAD_BODY_IDLE_TIMEOUT,
        )
        .await;

        assert!(result.is_err());
        assert!(stored_files(directory.path()).is_empty());
    }

    #[tokio::test]
    async fn idle_body_timeout_discards_stage_and_releases_upload_slot() {
        let directory = tempfile::tempdir().unwrap();
        let (storage, options) = upload_resources(directory.path()).await;
        let request = actix_web::test::TestRequest::default()
            .app_data(Data::new(UploadLimiter::new(NonZeroUsize::new(1).unwrap())))
            .to_http_request();
        let slot = UploadSlot::extract(&request).await.unwrap();
        let (_sender, body) = actix_http::h1::Payload::create(false);

        let result = {
            let result = stage_payload(
                &storage,
                body.into(),
                options.max_size,
                Duration::from_millis(10),
            )
            .await;

            drop(slot);

            result
        };
        let error = result.unwrap_err();

        assert_eq!(error.status_code(), StatusCode::REQUEST_TIMEOUT);
        assert!(stored_files(directory.path()).is_empty());
        assert!(UploadSlot::extract(&request).await.is_ok());
    }
}
