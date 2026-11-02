//! Helpers shared by the route tests: an application state over a test pool, a signed-in
//! session, and a request runner that decodes the JSON body.

use std::num::{NonZeroU32, NonZeroU64, NonZeroUsize};

use actix_web::{
    cookie::Cookie,
    dev::{Service, ServiceResponse},
    http::StatusCode,
    test,
};
use pp_core::config::PhotoDir;
use pp_database::{Database, auth::WorkspaceId};
use pp_storage::PhotoStorage;
use serde_json::{Value, json};
use sqlx::PgPool;
use tempfile::TempDir;

use crate::{
    api::ApiState,
    config::{AuthConfig, LimiterConfig, SessionConfig, UploadConfig},
    upload,
};

pub(crate) const ORIGIN: &str = "http://localhost:5173";

pub(crate) async fn upload_resources() -> (PhotoStorage, UploadConfig, TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let photo_dir: PhotoDir = directory.path().to_path_buf().try_into().unwrap();
    let storage = PhotoStorage::open(&photo_dir).await.unwrap();

    upload::install().unwrap();

    (
        storage,
        UploadConfig {
            max_size: NonZeroU64::new(67_108_864).unwrap(),
            max_pixels: NonZeroU64::new(100_000_000).unwrap(),
            concurrency: NonZeroUsize::new(4).unwrap(),
        },
        directory,
    )
}

pub(crate) async fn app_state(pool: PgPool) -> (ApiState, TempDir) {
    let (storage, upload_config, directory) = upload_resources().await;

    (state_with_upload(pool, storage, upload_config), directory)
}

pub(crate) fn state_with_upload(
    pool: PgPool,
    storage: PhotoStorage,
    upload_config: UploadConfig,
) -> ApiState {
    let session = SessionConfig {
        idle_timeout: "1h".parse().unwrap(),
        cookie_name: "pp_session".parse().unwrap(),
    };
    let auth = AuthConfig {
        app_origin: ORIGIN.parse().unwrap(),
    };
    let limiter = LimiterConfig {
        max_failures: NonZeroU32::new(10).unwrap(),
        window: "1m".parse().unwrap(),
        max_peers: NonZeroUsize::new(1024).unwrap(),
    };

    ApiState::new(
        Database::new(pool),
        session,
        auth,
        limiter,
        storage,
        upload_config,
    )
    .unwrap()
}

pub(crate) async fn call<S>(app: &S, request: test::TestRequest) -> (StatusCode, Value)
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let response = test::call_service(app, request.to_request()).await;
    let status = response.status();
    let bytes = test::read_body(response).await;
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);

    (status, body)
}

pub(crate) async fn sign_up<S>(app: &S) -> Cookie<'static>
where
    S: Service<actix_http::Request, Response = ServiceResponse, Error = actix_web::Error>,
{
    let request = test::TestRequest::post()
        .uri("/api/v1/auth/setup")
        .insert_header(("Origin", ORIGIN))
        .set_json(json!({"email": "person@example.test", "password": "a long test password"}))
        .to_request();
    let response = test::call_service(app, request).await;

    response
        .response()
        .cookies()
        .find(|cookie| cookie.name() == "pp_session")
        .unwrap()
        .into_owned()
}

/// Inserts a workspace other than the signed-in one, for cross-workspace denial tests.
pub(crate) async fn other_workspace(pool: &PgPool) -> WorkspaceId {
    sqlx::query_scalar("INSERT INTO workspaces(id) VALUES (gen_random_uuid()) RETURNING id")
        .fetch_one(pool)
        .await
        .unwrap()
}
