use std::sync::{Arc, Mutex, OnceLock};

use libvips::VipsApp;
use pp_storage::PhotoStorage;
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::config::{PhotoUploadConfig, PixelLimit, UploadSize};

static VIPS_APP: OnceLock<Mutex<Option<VipsApp>>> = OnceLock::new();

/// Failure to initialize bounded upload metadata support.
#[derive(Debug, Error)]
pub enum PhotoUploadError {
    #[error("image library initialization failed: {0}")]
    ImageLibrary(String),
    #[error("image library initialization state is poisoned")]
    InitializationLock,
}

/// Initializes libvips once with bounded internal resources.
pub(crate) fn initialize_image_library() -> Result<(), PhotoUploadError> {
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

/// Shared upload resources, opened and validated once during API startup.
pub struct PhotoUploadState {
    storage: PhotoStorage,
    max_size: UploadSize,
    max_pixels: PixelLimit,
    upload_slots: Arc<Semaphore>,
}

impl PhotoUploadState {
    pub fn new(storage: PhotoStorage, config: PhotoUploadConfig) -> Self {
        Self {
            storage,
            max_size: config.max_size,
            max_pixels: config.max_pixels,
            upload_slots: Arc::new(Semaphore::new(config.concurrency.get())),
        }
    }

    pub fn storage(&self) -> &PhotoStorage {
        &self.storage
    }

    pub const fn max_size(&self) -> UploadSize {
        self.max_size
    }

    pub const fn max_pixels(&self) -> PixelLimit {
        self.max_pixels
    }

    /// Reserves an upload slot without waiting; callers map exhaustion to service unavailable.
    pub fn try_acquire_upload_slot(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.upload_slots).try_acquire_owned().ok()
    }
}

#[cfg(test)]
mod tests {
    use std::num::{NonZeroU64, NonZeroUsize};

    use pp_core::config::PhotoDir;
    use pp_storage::PhotoStorage;

    use super::*;
    use crate::config::{PhotoUploadConfig, PixelLimit, UploadSize};

    #[tokio::test]
    async fn concurrent_upload_slots_fail_fast_at_the_configured_limit() {
        let directory = tempfile::tempdir().unwrap();
        let photo_dir: PhotoDir = directory.path().to_path_buf().try_into().unwrap();
        let storage = PhotoStorage::open(&photo_dir).await.unwrap();
        let state = PhotoUploadState::new(
            storage,
            PhotoUploadConfig {
                max_size: UploadSize::new(NonZeroU64::new(1024).unwrap()),
                max_pixels: PixelLimit::new(NonZeroU64::new(2048).unwrap()),
                concurrency: NonZeroUsize::new(4).unwrap(),
            },
        );
        let mut permits = (0..4)
            .map(|_| state.try_acquire_upload_slot().unwrap())
            .collect::<Vec<_>>();

        assert!(state.try_acquire_upload_slot().is_none());
        drop(permits.pop());
        assert!(state.try_acquire_upload_slot().is_some());
    }
}
