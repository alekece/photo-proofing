//! The client-facing error shared by every route.
//!
//! Messages are fixed strings that never echo input or internal details. The body is
//! `{"error": message}` plus a `"context"` array of extra details when a conversion has any,
//! such as the setting that failed validation.

use std::time::Duration;

use actix_web::{
    HttpResponse, ResponseError,
    error::JsonPayloadError,
    http::{StatusCode, header::RETRY_AFTER},
};
use pp_database::{error::DatabaseError, gallery::GalleryError};
use pp_storage::StorageError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{response::ResponseBuilderExt, routes::auth::AuthError};

#[derive(Debug, Error, Serialize, Deserialize)]
#[error("{message}")]
pub(crate) struct ErrorResponse {
    #[serde(skip)]
    status: StatusCode,
    #[serde(rename = "error")]
    message: String,
    /// Details a client may act on; the first entry is the most specific subject.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    context: Vec<String>,
    /// Sent as `Retry-After` when the client should simply try again later.
    #[serde(skip)]
    retry_after: Option<Duration>,
}

impl ErrorResponse {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            context: Vec::new(),
            retry_after: None,
        }
    }

    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not found")
    }

    pub fn unavailable() -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "service unavailable")
    }

    pub fn bad_request() -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid request")
    }

    pub fn request_timeout() -> Self {
        Self::new(StatusCode::REQUEST_TIMEOUT, "request body timed out")
    }

    pub fn unsupported_media_type() -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "expected an image/jpeg request body",
        )
    }

    pub fn payload_too_large() -> Self {
        Self::new(StatusCode::PAYLOAD_TOO_LARGE, "request body too large")
    }

    pub fn invalid_image() -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid JPEG image")
    }

    pub fn dimensions_too_large() -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "image dimensions exceed the configured limit",
        )
    }

    /// The server is busy: the client may retry after `retry_after`, sent in whole seconds.
    pub fn busy(retry_after: Duration) -> Self {
        Self {
            retry_after: Some(retry_after),
            ..Self::new(StatusCode::TOO_MANY_REQUESTS, "server busy, retry later")
        }
    }

    pub fn storage_full() -> Self {
        Self::new(StatusCode::INSUFFICIENT_STORAGE, "photo storage is full")
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
        Self::new(error.status_code(), error.to_string())
    }
}

impl From<GalleryError> for ErrorResponse {
    fn from(error: GalleryError) -> Self {
        let Some(setting) = error.setting() else {
            return Self::unavailable();
        };

        Self {
            context: vec![setting.to_string(), error.to_string()],
            ..Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid gallery setting")
        }
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
                Self::new(StatusCode::PAYLOAD_TOO_LARGE, "request body too large")
            }

            JsonPayloadError::ContentType => Self::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "expected a JSON request body",
            ),
            // Well-formed JSON with an unknown key, a null for a required field or a value
            // outside its type or validation.
            JsonPayloadError::Deserialize(error) if error.is_data() => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid request body")
            }
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
    use pp_storage::StorageError;

    use super::ErrorResponse;

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
}
