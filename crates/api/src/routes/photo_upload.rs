//! What the browser needs to know before sending photos.

use actix_web::{HttpResponse, get, web::Data};
use serde::Serialize;

use crate::{
    api::ApiState, error::ErrorResponse, extractors::identity::Authenticated,
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

#[get("/photo-upload/limits")]
pub async fn upload_limits(
    state: Data<ApiState>,
    _: Authenticated,
) -> Result<HttpResponse, ErrorResponse> {
    let config = state.upload_config();

    Ok(HttpResponse::Ok().no_store().json(UploadLimits {
        concurrency: config.concurrency.get(),
        max_size: config.max_size.get(),
        max_pixels: config.max_pixels.get(),
    }))
}

#[cfg(test)]
mod tests {
    use actix_web::{cookie::Cookie, http::StatusCode, test};
    use serde_json::json;
    use sqlx::PgPool;

    use crate::{
        routes::galleries::photos::tests::{app, call_request, upload_config},
        tests::sign_up,
    };

    fn limits_request(cookie: Option<&Cookie<'static>>) -> actix_http::Request {
        let request = test::TestRequest::get().uri("/api/v1/photo-upload/limits");

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
