//! What the browser needs to know before sending photos.

use actix_web::{HttpResponse, get, web::Data};
use serde::Serialize;

use crate::{
    api::ApiState, config::UploadConfig, error::ErrorResponse, extractors::identity::Authenticated,
    response::ResponseBuilderExt,
};

/// The upload bounds a client should respect; the server enforces them anyway.
#[derive(Serialize)]
struct UploadLimits {
    /// Uploads the server accepts at once, shared by every signed-in page.
    concurrency: usize,
    max_size: u64,
    max_pixels: u64,
}

impl From<&UploadConfig> for UploadLimits {
    fn from(config: &UploadConfig) -> Self {
        Self {
            concurrency: config.concurrency.get(),
            max_size: config.max_size.get(),
            max_pixels: config.max_pixels.get(),
        }
    }
}

#[get("/upload/limits")]
pub async fn upload_limits(
    state: Data<ApiState>,
    _: Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    Ok(HttpResponse::Ok()
        .no_store()
        .json(UploadLimits::from(state.upload_config())))
}

#[cfg(test)]
mod tests {
    use actix_web::{cookie::Cookie, http::StatusCode, test};
    use serde_json::json;
    use sqlx::PgPool;

    use crate::{
        tests::sign_up,
        tests::{app, call_request, upload_config},
    };

    fn limits_request(cookie: Option<&Cookie<'static>>) -> actix_http::Request {
        let request = test::TestRequest::get().uri("/api/v1/upload/limits");

        match cookie {
            Some(cookie) => request.cookie(cookie.clone()).to_request(),
            None => request.to_request(),
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn limits_require_a_session(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool, directory.path(), upload_config(1024, 2048)).await;
        let (status, _) = call_request(&app, limits_request(None)).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn limits_return_the_upload_config(pool: PgPool) {
        let directory = tempfile::tempdir().unwrap();
        let app = app(pool, directory.path(), upload_config(1024, 2048)).await;
        let cookie = sign_up(&app).await;
        let (status, body) = call_request(&app, limits_request(Some(&cookie))).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({"concurrency": 4, "max_size": 1024, "max_pixels": 2048})
        );
    }
}
