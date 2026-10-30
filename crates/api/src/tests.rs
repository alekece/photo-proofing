//! Helpers shared by the route tests: an application state over a test pool, a signed-in
//! session, and a request runner that decodes the JSON body.

use std::num::{NonZeroU32, NonZeroUsize};

use actix_web::{
    cookie::Cookie,
    dev::{Service, ServiceResponse},
    http::StatusCode,
    test,
};
use pp_database::{Database, auth::WorkspaceId};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    api::ApiState,
    config::{AuthConfig, LimiterConfig, SessionConfig},
};

pub(crate) const ORIGIN: &str = "http://localhost:5173";

pub(crate) fn app_state(pool: PgPool) -> ApiState {
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

    ApiState::new(Database::new(pool), session, auth, limiter).unwrap()
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
