//! Shared fail-fast upload admission, independent of validation and application state.

use std::{
    future::{Ready, ready},
    sync::Arc,
    time::Duration,
};

use actix_web::{FromRequest, HttpRequest, dev::Payload, web::Data};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{config::UploadConfig, error::ErrorResponse};

/// One process-wide capacity limit, registered as shared application data across workers.
pub(crate) struct UploadLimiter {
    slots: Arc<Semaphore>,
    /// Wait announced to a refused upload; slots free up as other uploads finish.
    retry_after: Duration,
}

impl From<&UploadConfig> for UploadLimiter {
    fn from(config: &UploadConfig) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(config.concurrency.get())),
            retry_after: config.retry_after.get(),
        }
    }
}

/// Reserves capacity until the handler returns or is cancelled; never reads a request body.
///
/// Extraction fails fast with 429 and `Retry-After`, before handler authorization or header
/// validation.
#[must_use]
pub(crate) struct UploadSlot {
    _permit: OwnedSemaphorePermit,
}

impl FromRequest for UploadSlot {
    type Error = ErrorResponse;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(request: &HttpRequest, _: &mut Payload) -> Self::Future {
        let Some(limiter) = request.app_data::<Data<UploadLimiter>>() else {
            tracing::error!("upload request is missing its shared limiter");

            return ready(Err(ErrorResponse::unavailable()));
        };
        let result = Arc::clone(&limiter.slots)
            .try_acquire_owned()
            .map(|permit| Self { _permit: permit })
            .map_err(|_| ErrorResponse::busy(limiter.retry_after));

        ready(result)
    }
}

#[cfg(test)]
mod tests {
    use actix_web::{
        FromRequest, ResponseError,
        http::{StatusCode, header::RETRY_AFTER},
        test,
        web::Data,
    };

    use super::{UploadLimiter, UploadSlot};
    use crate::routes::galleries::photos::tests::upload_config;

    #[actix_web::test]
    async fn cloned_applications_share_capacity_without_api_state() {
        let mut config = upload_config(1024, 2048);

        config.retry_after = "3s".parse().unwrap();

        let limiter = Data::new(UploadLimiter::from(&config));
        let first = test::TestRequest::default()
            .app_data(limiter.clone())
            .to_http_request();
        let second = test::TestRequest::default()
            .app_data(limiter)
            .to_http_request();
        let mut slots = Vec::new();

        for _ in 0..4 {
            slots.push(UploadSlot::extract(&first).await.unwrap());
        }

        let error = UploadSlot::extract(&second).await.err().unwrap();
        let response = error.error_response();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(response.headers().get(RETRY_AFTER).unwrap(), "3");
        drop(slots.pop());
        let slot = UploadSlot::extract(&second).await.unwrap();

        assert!(UploadSlot::extract(&first).await.is_err());
        drop(slot);
        assert!(UploadSlot::extract(&first).await.is_ok());
    }

    #[actix_web::test]
    async fn missing_limiter_is_an_infrastructure_error() {
        let request = test::TestRequest::default().to_http_request();
        let error = UploadSlot::extract(&request).await.err().unwrap();

        assert_eq!(error.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
