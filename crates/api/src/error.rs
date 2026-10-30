//! The client-facing error shared by every route.
//!
//! Messages are fixed strings that never echo input or internal details. The body is
//! `{"error": message}` plus `"field"` when one setting caused a validation failure.

use actix_web::{
    HttpResponse, ResponseError,
    error::JsonPayloadError,
    http::{StatusCode, header},
};
use pp_database::{SqlError, gallery::GalleryError};
use serde_json::{Map, Value, json};

use crate::routes::auth::AuthError;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub(crate) struct ErrorResponse {
    status: StatusCode,
    message: String,
    field: Option<String>,
}

impl ErrorResponse {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            field: None,
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
}

impl ResponseError for ErrorResponse {
    fn status_code(&self) -> StatusCode {
        self.status
    }

    fn error_response(&self) -> HttpResponse {
        let mut body = Map::new();

        body.insert("error".into(), json!(&self.message));

        if let Some(field) = &self.field {
            body.insert("field".into(), Value::String(field.clone()));
        }

        HttpResponse::build(self.status)
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(body)
    }
}

impl From<AuthError> for ErrorResponse {
    fn from(error: AuthError) -> Self {
        Self::new(error.status_code(), error.to_string())
    }
}

impl From<GalleryError> for ErrorResponse {
    fn from(error: GalleryError) -> Self {
        match error {
            GalleryError::Invalid(setting) => Self {
                status: StatusCode::UNPROCESSABLE_ENTITY,
                message: "invalid gallery setting".into(),
                field: Some(setting.to_string()),
            },
            GalleryError::Database(_) => Self::unavailable(),
        }
    }
}

impl From<SqlError> for ErrorResponse {
    fn from(_: SqlError) -> Self {
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
            // Well-formed JSON with an unknown key, a null for a required setting or a value
            // outside its type.
            JsonPayloadError::Deserialize(error) if error.is_data() => {
                Self::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid gallery settings")
            }
            _ => Self::bad_request(),
        }
    }
}
