//! Bounded JPEG header inspection; libvips work runs outside the async runtime.
//!
//! Inspection reads dimensions, not the complete compressed pixel stream. Initialization
//! disables operation caches and bounds libvips concurrency for the whole process.

use std::{
    fs::File,
    io::{self, Read},
    num::{NonZeroU32, NonZeroU64},
    path::Path,
    sync::{Mutex, OnceLock},
};

use libvips::{VipsApp, VipsImage};
use pp_database::photo::PhotoDimensions;
use pp_storage::StorageError;
use thiserror::Error;

use crate::error::ErrorResponse;

static VIPS_APP: OnceLock<Mutex<Option<VipsApp>>> = OnceLock::new();

/// Failure to initialize bounded JPEG inspection.
#[derive(Debug, Error)]
pub enum InitializationError {
    #[error("image library initialization failed: {0}")]
    ImageLibrary(String),
    #[error("image library initialization state is poisoned")]
    InitializationLock,
}

pub(crate) fn install() -> Result<(), InitializationError> {
    let app = VIPS_APP.get_or_init(|| Mutex::new(None));
    let mut app = app
        .lock()
        .map_err(|_| InitializationError::InitializationLock)?;

    if app.is_none() {
        let initialized = VipsApp::new("photo-proofing-api", false)
            .map_err(|error| InitializationError::ImageLibrary(format!("{error:?}")))?;

        initialized.concurrency_set(1);
        initialized.cache_set_max(0);
        initialized.cache_set_max_mem(0);
        initialized.cache_set_max_files(0);
        *app = Some(initialized);
    }

    Ok(())
}

pub(crate) async fn inspect(
    path: &Path,
    max_pixels: NonZeroU64,
) -> Result<PhotoDimensions, ErrorResponse> {
    let path = path.to_owned();

    tokio::task::spawn_blocking(move || {
        let mut file = File::open(&path).map_err(StorageError::from)?;
        let mut signature = [0; 3];

        if let Err(error) = file.read_exact(&mut signature) {
            if error.kind() == io::ErrorKind::UnexpectedEof {
                return Err(ErrorResponse::invalid_image());
            }

            return Err(StorageError::from(error).into());
        }

        if signature != [0xff, 0xd8, 0xff] {
            return Err(ErrorResponse::invalid_image());
        }

        let path = path.to_str().ok_or_else(|| {
            tracing::error!("JPEG staging path is not UTF-8");
            ErrorResponse::unavailable()
        })?;
        // The generic loader accepts this without version-specific JPEG options.
        let path = format!("{path}[fail-on=truncated]");
        let image = VipsImage::new_from_file(&path).map_err(|_| ErrorResponse::invalid_image())?;
        let width = u32::try_from(image.get_width())
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(ErrorResponse::invalid_image)?;
        let height = u32::try_from(image.get_height())
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(ErrorResponse::invalid_image)?;
        // EXIF orientations 5 to 8 turn the photo a quarter, so it displays with swapped sides.
        let (width, height) = if (5..=8).contains(&image.get_orientation()) {
            (height, width)
        } else {
            (width, height)
        };

        validate_pixel_count(u64::from(width.get()), u64::from(height.get()), max_pixels)?;

        Ok(PhotoDimensions { width, height })
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "JPEG metadata inspection task failed");
        ErrorResponse::unavailable()
    })?
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

#[cfg(test)]
pub(crate) mod tests {
    use std::{fs, num::NonZeroU64};

    use actix_web::{ResponseError, http::StatusCode};
    use libvips::ops::{black, jpegsave_buffer};

    use super::{inspect, install, validate_pixel_count};

    #[tokio::test]
    async fn inspection_reads_dimensions_and_enforces_the_pixel_bound() {
        install().unwrap();

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo");
        let jpeg = jpegsave_buffer(&black(20, 10).unwrap()).unwrap();

        fs::write(&path, &jpeg).unwrap();

        let dimensions = inspect(&path, NonZeroU64::new(200).unwrap()).await.unwrap();

        assert_eq!(dimensions.width.get(), 20);
        assert_eq!(dimensions.height.get(), 10);
        assert_eq!(
            inspect(&path, NonZeroU64::new(199).unwrap())
                .await
                .unwrap_err()
                .status_code(),
            StatusCode::UNPROCESSABLE_ENTITY
        );
        assert_eq!(fs::read(path).unwrap(), jpeg);
    }

    #[tokio::test]
    async fn inspection_rejects_non_jpeg_and_short_headers() {
        install().unwrap();

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo");

        for bytes in [b"not a JPEG".as_slice(), b"\xff\xd8", b"\xff\xd8\xff"] {
            fs::write(&path, bytes).unwrap();

            let error = inspect(&path, NonZeroU64::new(200).unwrap())
                .await
                .unwrap_err();

            assert_eq!(error.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
        }
    }

    #[tokio::test]
    async fn inspection_applies_the_exif_orientation() {
        install().unwrap();

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("photo");
        let jpeg = jpegsave_buffer(&black(20, 10).unwrap()).unwrap();
        let mut cases = vec![(jpeg.clone(), (20, 10))];

        for (orientation, expected) in [
            (6, (10, 20)),
            (8, (10, 20)),
            (3, (20, 10)),
            (0, (20, 10)),
            (9, (20, 10)),
        ] {
            let source = with_segment(&jpeg, 0xe1, &orientation_exif(orientation));

            cases.push((source, expected));
        }

        for (bytes, (width, height)) in cases {
            fs::write(&path, bytes).unwrap();

            let dimensions = inspect(&path, NonZeroU64::new(200).unwrap()).await.unwrap();

            assert_eq!(
                (dimensions.width.get(), dimensions.height.get()),
                (width, height)
            );
        }
    }

    pub(crate) fn with_segment(jpeg: &[u8], marker: u8, payload: &[u8]) -> Vec<u8> {
        let length = u16::try_from(payload.len() + 2).unwrap();
        let mut out = jpeg[..2].to_vec();

        out.extend([0xff, marker]);
        out.extend(length.to_be_bytes());
        out.extend(payload);
        out.extend(&jpeg[2..]);

        out
    }

    /// Big-endian EXIF block whose IFD0 holds only the orientation tag.
    pub(crate) fn orientation_exif(orientation: u16) -> Vec<u8> {
        let mut exif = b"Exif\0\0MM\0\x2a\0\0\0\x08".to_vec();

        exif.extend(1_u16.to_be_bytes());
        exif.extend(0x0112_u16.to_be_bytes());
        exif.extend(3_u16.to_be_bytes());
        exif.extend(1_u32.to_be_bytes());
        exif.extend((u32::from(orientation) << 16).to_be_bytes());
        exif.extend(0_u32.to_be_bytes());

        exif
    }

    #[test]
    fn overflowing_pixel_count_is_rejected() {
        assert!(validate_pixel_count(u64::MAX, 2, NonZeroU64::new(2048).unwrap()).is_err());
    }
}
