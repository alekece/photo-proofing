//! The client-facing error shared by every route.
//!
//! Messages are fixed strings that never echo input or internal details. The body is
//! `{"error": message}` plus a `"context"` array of extra details when a conversion has any,
//! such as the setting that failed validation.

use actix_web::{HttpResponse, ResponseError, error::JsonPayloadError, http::StatusCode};
use pp_database::{error::DatabaseError, gallery::GalleryError};
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
}

impl ErrorResponse {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            context: Vec::new(),
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

    pub fn storage_full() -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "photo storage is full")
    }
}

impl ResponseError for ErrorResponse {
    fn status_code(&self) -> StatusCode {
        self.status
    }

    fn error_response(&self) -> HttpResponse {
        // Sent on every error so a rejected private link cannot be told apart from another by
        // its headers.
        HttpResponse::build(self.status)
            .no_store()
            .no_referrer()
            .json(self)
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
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: "invalid gallery setting".into(),
            context: vec![setting.to_string(), error.to_string()],
        }
    }
}

impl From<DatabaseError> for ErrorResponse {
    fn from(_: DatabaseError) -> Self {
        Self::unavailable()
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
