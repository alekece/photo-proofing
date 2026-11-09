//! The client-facing error shared by every route.
//!
//! Each error carries a stable [`ErrorCode`] the web translates; the body is
//! `{"code": code}`. Debug builds add `"context": [message]`, a fixed English sentence that
//! never echoes input or internal details, to help whoever reads the response.

use std::time::Duration;

use actix_web::{
    HttpResponse, ResponseError,
    error::JsonPayloadError,
    http::{StatusCode, header::RETRY_AFTER},
};
use pp_database::{
    error::DatabaseError,
    export::{ExportFailure, ExportStatus},
    gallery::{GalleryError, GallerySetting},
    photo::FavoriteChange,
    settings::SettingsError,
};
use pp_storage::StorageError;
use serde::Serialize;
use thiserror::Error;

use crate::{response::ResponseBuilderExt, routes::auth::AuthError};

/// What went wrong, as a stable name the web maps to a translated message. Names are frozen:
/// renaming one breaks every client that knows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ErrorCode {
    Unauthorized,
    InvalidLogin,
    ForbiddenOrigin,
    NotFound,
    SetupClosed,
    GalleryDelivered,
    GalleryNotShared,
    SelectionEmpty,
    ZipNotReady,
    ZipInProgress,
    RequestTimeout,
    PayloadTooLarge,
    SelectionTooLarge,
    UnsupportedMediaType,
    BadRequest,
    InvalidImage,
    DimensionsTooLarge,
    InvalidLogo,
    LogoTooLarge,
    LogoRequired,
    InvalidTitle,
    InvalidIncludedPhotos,
    InvalidStudioName,
    InvalidPosition,
    InvalidSize,
    TooManyAttempts,
    Busy,
    StorageFull,
    Unavailable,
    PhotoRemoved,
    ExportFailed,
    InvalidRequestBody,
}

impl From<ExportFailure> for ErrorCode {
    fn from(failure: ExportFailure) -> Self {
        match failure {
            ExportFailure::AttemptsExhausted => Self::ExportFailed,
            ExportFailure::PhotoRemoved => Self::PhotoRemoved,
            ExportFailure::DiskFull => Self::StorageFull,
        }
    }
}

/// An export's status as the photographer's UI receives it; a failure is an [`ErrorCode`].
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ExportStatusResponse {
    Preparing,
    Ready,
    Failed { code: ErrorCode },
}

impl From<ExportStatus> for ExportStatusResponse {
    fn from(status: ExportStatus) -> Self {
        match status {
            ExportStatus::Preparing => Self::Preparing,
            ExportStatus::Ready => Self::Ready,
            ExportStatus::Failed(failure) => Self::Failed {
                code: failure.into(),
            },
        }
    }
}

#[derive(Debug, Error, Serialize)]
#[error("{}", .context.first().map_or("", String::as_str))]
pub(crate) struct ErrorResponse {
    #[serde(skip)]
    status: StatusCode,
    code: ErrorCode,
    /// English detail for debug builds only; release builds never send it.
    #[cfg_attr(not(debug_assertions), serde(skip))]
    context: Vec<String>,
    /// Sent as `Retry-After` when the client should simply try again later.
    #[serde(skip)]
    retry_after: Option<Duration>,
}

impl ErrorResponse {
    pub fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            context: vec![message.into()],
            retry_after: None,
        }
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, "not found")
    }

    pub fn gallery_delivered() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ErrorCode::GalleryDelivered,
            "gallery already delivered",
        )
    }

    pub fn gallery_not_shared() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ErrorCode::GalleryNotShared,
            "gallery is not shared",
        )
    }

    pub fn zip_not_ready() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ErrorCode::ZipNotReady,
            "the ZIP is not ready",
        )
    }

    pub fn zip_in_progress() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ErrorCode::ZipInProgress,
            "a ZIP is already being prepared",
        )
    }

    /// Succeeds only when the change was applied; a photo outside the gallery is a 404 and a
    /// delivered gallery a 409.
    pub fn ensure_favorite_applied(change: FavoriteChange) -> Result<(), Self> {
        match change {
            FavoriteChange::Applied => Ok(()),
            FavoriteChange::NotFound => Err(Self::not_found()),
            FavoriteChange::Delivered => Err(Self::gallery_delivered()),
        }
    }

    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            "service unavailable",
        )
    }

    pub fn bad_request() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::BadRequest,
            "invalid request",
        )
    }

    pub fn request_timeout() -> Self {
        Self::new(
            StatusCode::REQUEST_TIMEOUT,
            ErrorCode::RequestTimeout,
            "request body timed out",
        )
    }

    pub fn unsupported_media_type() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ErrorCode::UnsupportedMediaType,
            "expected an image/jpeg request body",
        )
    }

    pub fn payload_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::PayloadTooLarge,
            "request body too large",
        )
    }

    pub fn empty_selection() -> Self {
        Self::new(
            StatusCode::CONFLICT,
            ErrorCode::SelectionEmpty,
            "the selection is empty",
        )
    }

    pub fn selection_too_large() -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::SelectionTooLarge,
            "the selection is too large to download as one ZIP",
        )
    }

    pub fn invalid_image() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidImage,
            "invalid JPEG image",
        )
    }

    pub fn dimensions_too_large() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::DimensionsTooLarge,
            "image dimensions exceed the configured limit",
        )
    }

    pub fn invalid_logo() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidLogo,
            "invalid logo image",
        )
    }

    pub fn logo_too_large() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::LogoTooLarge,
            "logo is too large",
        )
    }

    /// The server is busy: the client may retry after `retry_after`, sent in whole seconds.
    pub fn busy(retry_after: Duration) -> Self {
        Self {
            retry_after: Some(retry_after),
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                ErrorCode::Busy,
                "server busy, retry later",
            )
        }
    }

    pub fn storage_full() -> Self {
        Self::new(
            StatusCode::INSUFFICIENT_STORAGE,
            ErrorCode::StorageFull,
            "photo storage is full",
        )
    }
}

impl ResponseError for ErrorResponse {
    fn status_code(&self) -> StatusCode {
        self.status
    }

    fn error_response(&self) -> HttpResponse {
        // Sent on every error so a rejected private link cannot be told apart from another by
        // its headers.
        let mut response = HttpResponse::build(self.status);

        if let Some(retry_after) = self.retry_after {
            let seconds = retry_after.as_secs_f64().ceil().max(1.0);

            response.insert_header((RETRY_AFTER, format!("{seconds}")));
        }

        response.no_store().no_referrer().json(self)
    }
}

impl From<AuthError> for ErrorResponse {
    fn from(error: AuthError) -> Self {
        let code = match error {
            AuthError::Unavailable => ErrorCode::Unavailable,
            AuthError::Limited => ErrorCode::TooManyAttempts,
            AuthError::Unauthorized => ErrorCode::Unauthorized,
            AuthError::ForbiddenOrigin => ErrorCode::ForbiddenOrigin,
            AuthError::InvalidLogin => ErrorCode::InvalidLogin,
            AuthError::SetupClosed => ErrorCode::SetupClosed,
        };

        Self::new(error.status_code(), code, error.to_string())
    }
}

impl From<GalleryError> for ErrorResponse {
    fn from(error: GalleryError) -> Self {
        let Some(setting) = error.setting() else {
            return Self::unavailable();
        };

        let code = match setting {
            GallerySetting::Title => ErrorCode::InvalidTitle,
            GallerySetting::IncludedPhotos => ErrorCode::InvalidIncludedPhotos,
        };

        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, error.to_string())
    }
}

impl From<SettingsError> for ErrorResponse {
    fn from(error: SettingsError) -> Self {
        let code = match error {
            SettingsError::NameRequired | SettingsError::NameTooLong => {
                ErrorCode::InvalidStudioName
            }
            SettingsError::LogoRequired => ErrorCode::LogoRequired,
            SettingsError::GridCell(_) => ErrorCode::InvalidPosition,
            SettingsError::SizeStep(_) => ErrorCode::InvalidSize,
            SettingsError::Database(_) => return Self::unavailable(),
        };

        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, error.to_string())
    }
}

impl From<DatabaseError> for ErrorResponse {
    fn from(_: DatabaseError) -> Self {
        Self::unavailable()
    }
}

impl From<StorageError> for ErrorResponse {
    fn from(error: StorageError) -> Self {
        tracing::error!(%error, "photo storage operation failed");

        match error {
            StorageError::DiskFull(_) => Self::storage_full(),
            _ => Self::unavailable(),
        }
    }
}

impl From<JsonPayloadError> for ErrorResponse {
    fn from(error: JsonPayloadError) -> Self {
        match error {
            JsonPayloadError::Overflow { .. } | JsonPayloadError::OverflowKnownLength { .. } => {
                Self::payload_too_large()
            }

            JsonPayloadError::ContentType => Self::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ErrorCode::UnsupportedMediaType,
                "expected a JSON request body",
            ),
            // Well-formed JSON with an unknown key, a null for a required field or a value
            // outside its type or validation.
            JsonPayloadError::Deserialize(error) if error.is_data() => Self::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidRequestBody,
                "invalid request body",
            ),
            _ => Self::bad_request(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io, time::Duration};

    use actix_web::{
        ResponseError,
        http::{StatusCode, header::RETRY_AFTER},
    };
    use pp_database::{
        export::{ExportFailure, ExportStatus},
        gallery::{GalleryError, GallerySetting},
        settings::SettingsError,
    };
    use pp_storage::StorageError;
    use serde_json::json;

    use super::{ErrorCode, ErrorResponse, ExportStatusResponse};

    #[test]
    fn storage_errors_keep_their_http_contract_without_exposing_causes() {
        let full = ErrorResponse::from(StorageError::DiskFull(io::Error::from(
            io::ErrorKind::StorageFull,
        )));
        let failed = ErrorResponse::from(StorageError::Io(io::Error::other(
            "/private/photo/path failed",
        )));

        assert_eq!(full.status_code(), StatusCode::INSUFFICIENT_STORAGE);
        assert_eq!(full.to_string(), "photo storage is full");
        assert_eq!(failed.status_code(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(failed.to_string(), "service unavailable");
        assert!(!serde_json::to_string(&failed).unwrap().contains("/private"));
    }

    #[test]
    fn busy_asks_to_retry_after_whole_seconds() {
        for (retry_after, header) in [
            (Duration::from_secs(3), "3"),
            (Duration::from_millis(1500), "2"),
            (Duration::ZERO, "1"),
        ] {
            let response = ErrorResponse::busy(retry_after).error_response();

            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
            assert_eq!(response.headers().get(RETRY_AFTER).unwrap(), header);
        }

        let other = ErrorResponse::unavailable().error_response();

        assert!(other.headers().get(RETRY_AFTER).is_none());
    }

    #[test]
    fn error_codes_are_frozen() {
        let codes = [
            (ErrorCode::Unauthorized, "unauthorized"),
            (ErrorCode::InvalidLogin, "invalid_login"),
            (ErrorCode::ForbiddenOrigin, "forbidden_origin"),
            (ErrorCode::NotFound, "not_found"),
            (ErrorCode::SetupClosed, "setup_closed"),
            (ErrorCode::GalleryDelivered, "gallery_delivered"),
            (ErrorCode::GalleryNotShared, "gallery_not_shared"),
            (ErrorCode::SelectionEmpty, "selection_empty"),
            (ErrorCode::ZipNotReady, "zip_not_ready"),
            (ErrorCode::ZipInProgress, "zip_in_progress"),
            (ErrorCode::RequestTimeout, "request_timeout"),
            (ErrorCode::PayloadTooLarge, "payload_too_large"),
            (ErrorCode::SelectionTooLarge, "selection_too_large"),
            (ErrorCode::UnsupportedMediaType, "unsupported_media_type"),
            (ErrorCode::BadRequest, "bad_request"),
            (ErrorCode::InvalidImage, "invalid_image"),
            (ErrorCode::DimensionsTooLarge, "dimensions_too_large"),
            (ErrorCode::InvalidLogo, "invalid_logo"),
            (ErrorCode::LogoTooLarge, "logo_too_large"),
            (ErrorCode::LogoRequired, "logo_required"),
            (ErrorCode::InvalidTitle, "invalid_title"),
            (ErrorCode::InvalidIncludedPhotos, "invalid_included_photos"),
            (ErrorCode::InvalidStudioName, "invalid_studio_name"),
            (ErrorCode::InvalidPosition, "invalid_position"),
            (ErrorCode::InvalidSize, "invalid_size"),
            (ErrorCode::TooManyAttempts, "too_many_attempts"),
            (ErrorCode::Busy, "busy"),
            (ErrorCode::StorageFull, "storage_full"),
            (ErrorCode::Unavailable, "unavailable"),
            (ErrorCode::PhotoRemoved, "photo_removed"),
            (ErrorCode::ExportFailed, "export_failed"),
            (ErrorCode::InvalidRequestBody, "invalid_request_body"),
        ];

        for (code, name) in codes {
            assert_eq!(serde_json::to_value(code).unwrap(), name);
        }
    }

    #[test]
    fn the_body_has_a_code_and_never_the_old_error_key() {
        let body = serde_json::to_value(ErrorResponse::not_found()).unwrap();

        assert_eq!(body["code"], "not_found");
        assert!(body.get("error").is_none());
        assert_eq!(
            body.get("context").is_some(),
            cfg!(debug_assertions),
            "context is a debug-build aid only"
        );
    }

    #[test]
    fn setting_errors_choose_a_code_per_field() {
        let title = ErrorResponse::from(GalleryError::LengthOverflow(GallerySetting::Title));
        let included = ErrorResponse::from(GalleryError::InvalidCharacters(
            GallerySetting::IncludedPhotos,
        ));
        let studio = ErrorResponse::from(SettingsError::NameRequired);
        let logo = ErrorResponse::from(SettingsError::LogoRequired);

        assert_eq!(
            serde_json::to_value(title).unwrap()["code"],
            "invalid_title"
        );
        assert_eq!(
            serde_json::to_value(included).unwrap()["code"],
            "invalid_included_photos"
        );
        assert_eq!(
            serde_json::to_value(studio).unwrap()["code"],
            "invalid_studio_name"
        );
        assert_eq!(serde_json::to_value(logo).unwrap()["code"], "logo_required");
    }

    #[test]
    fn export_failures_serialize_as_codes_without_the_stored_text() {
        for (failure, code) in [
            (ExportFailure::AttemptsExhausted, "export_failed"),
            (ExportFailure::PhotoRemoved, "photo_removed"),
            (ExportFailure::DiskFull, "storage_full"),
        ] {
            let status = ExportStatusResponse::from(ExportStatus::Failed(failure));

            assert_eq!(
                serde_json::to_value(status).unwrap(),
                json!({"kind": "failed", "code": code})
            );
        }

        assert_eq!(
            serde_json::to_value(ExportStatusResponse::from(ExportStatus::Ready)).unwrap(),
            json!({"kind": "ready"})
        );
    }
}
