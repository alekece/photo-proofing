//! Account setup and cookie-backed session endpoints under `/api/v1/auth`.
//!
//! - `GET /status` is passive: it reports whether setup is open and who the cookie identifies,
//!   but never extends a session, so polling cannot keep one alive.
//! - `POST /setup` creates the only account and its first session, hashing the password off
//!   the async executor. Setup closes once an account exists.
//! - `POST /login` verifies credentials and issues a new session token. Unknown emails are
//!   verified against a dummy hash so response time does not reveal which emails exist.
//! - `POST /renew` extends the idle expiry; the browser calls it only on real user activity.
//! - `POST /logout` deletes the session row, which invalidates every copy of the cookie.
//!
//! The cookie holds a random opaque [`SessionToken`]; only its digest is stored, so a database
//! leak does not yield usable sessions. Every state-changing request must carry the exact
//! configured `Origin`, which the API-wide [`SameOrigin`](crate::middlewares::origin::SameOrigin)
//! middleware enforces as the CSRF defence for this cookie-authenticated API before any
//! extractor reads the request. Sign-in routes also pass the failed-attempt limiter
//! ([`Attempt`]); [`FailedAttempts`] counts their failed responses, including credential bodies
//! rejected while being parsed. Request bodies are validated while being parsed
//! ([`Credentials`]), and policy and limiter state live in [`ApiState`].

use std::time::SystemTime;

use actix_web::{
    HttpResponse, ResponseError, get,
    http::StatusCode,
    post,
    web::{self, Data, Json, ServiceConfig},
};
use pp_core::{email::Email, time::Millis};
use pp_database::auth::{AuthRepository, Identity};
use serde::{Deserialize, Serialize};

use crate::{
    api::ApiState,
    credentials::{Password, PasswordHash},
    extractors::{
        attempt::Attempt,
        session_token::{SessionToken, TokenHash},
    },
    middlewares::failed_attempts::FailedAttempts,
    response::ResponseBuilderExt,
};

/// A safe, client-facing failure: each variant has a fixed message that never echoes input
/// or internal details. Rendered as `{"error": message}` with `Cache-Control: no-store`.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AuthError {
    #[error("authentication service unavailable")]
    Unavailable,
    #[error("too many sign-in attempts; try again later")]
    Limited,
    #[error("session expired or sign-in required")]
    Unauthorized,
    #[error("request origin is not allowed")]
    ForbiddenOrigin,
    #[error("invalid email or password")]
    InvalidLogin,
    #[error("initial setup is already complete")]
    SetupClosed,
}

impl ResponseError for AuthError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Limited => StatusCode::TOO_MANY_REQUESTS,
            Self::Unauthorized | Self::InvalidLogin => StatusCode::UNAUTHORIZED,
            Self::ForbiddenOrigin => StatusCode::FORBIDDEN,
            Self::SetupClosed => StatusCode::CONFLICT,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .no_store()
            .json(serde_json::json!({"error": self.to_string()}))
    }
}

/// Request body of `setup` and `login`. Parsing normalizes the email and bounds the password,
/// so a handler only ever sees valid credentials.
#[derive(Deserialize)]
struct Credentials {
    email: Email,
    password: Password,
}

#[derive(Serialize)]
struct AuthStatus {
    setup_available: bool,
    identity: Option<Identity>,
    #[serde(rename = "idle_timeout_ms")]
    idle_timeout: Millis,
}

/// Registers the auth routes. Their credential bodies are parsed by the API-wide JSON
/// configuration, and only the credential routes count failed attempts.
pub fn configure(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .service(report_status)
            .service(logout)
            .service(renew)
            .service(
                web::scope("")
                    .wrap(FailedAttempts::new())
                    .service(setup)
                    .service(login),
            ),
    );
}

#[get("/status")]
async fn report_status(
    state: Data<ApiState>,
    session: Option<SessionToken>,
) -> Result<HttpResponse, AuthError> {
    let setup_available = state
        .database()
        .repository::<AuthRepository>()
        .setup_available()
        .await
        .map_err(|_| AuthError::Unavailable)?;

    let identity = match session {
        Some(token) => state
            .database()
            .repository::<AuthRepository>()
            .authenticate(token.digest().as_bytes(), SystemTime::now())
            .await
            .map_err(|_| AuthError::Unavailable)?,
        None => None,
    };

    Ok(HttpResponse::Ok().no_store().json(AuthStatus {
        setup_available,
        identity,
        idle_timeout: state.idle_timeout().into(),
    }))
}

#[post("/setup")]
async fn setup(
    state: Data<ApiState>,
    _attempt: Attempt,
    credentials: Json<Credentials>,
) -> Result<HttpResponse, AuthError> {
    let Credentials { email, password } = credentials.into_inner();
    let permit = state.try_acquire_hash_slot().ok_or(AuthError::Limited)?;

    let password_hash = web::block(move || {
        let _permit = permit;
        PasswordHash::new(password.as_str())
    })
    .await
    .map_err(|_| AuthError::Unavailable)?
    .map_err(|_| AuthError::Unavailable)?;

    // Concurrent setups can both reach here; the database allows only one account and
    // `create_account` returns `None` to the loser, which becomes the 409 below.
    let token = SessionToken::generate();
    let expires_at = state
        .session_expiry(SystemTime::now())
        .ok_or(AuthError::Unavailable)?;
    let id = state
        .database()
        .repository::<AuthRepository>()
        .create_account(
            &email,
            password_hash.as_str(),
            token.digest().as_bytes(),
            expires_at,
        )
        .await
        .map_err(|_| AuthError::Unavailable)?
        .ok_or(AuthError::SetupClosed)?;

    Ok(HttpResponse::Ok()
        .no_store()
        .cookie(state.session_cookie(&token))
        .json(id))
}

#[post("/login")]
async fn login(
    state: Data<ApiState>,
    session: Option<SessionToken>,
    _attempt: Attempt,
    credentials: Json<Credentials>,
) -> Result<HttpResponse, AuthError> {
    let Credentials { email, password } = credentials.into_inner();
    let account = state
        .database()
        .repository::<AuthRepository>()
        .account_by_email(&email)
        .await
        .map_err(|_| AuthError::Unavailable)?;
    // Verify against a dummy hash when the email is unknown so both cases cost the same.
    let stored_hash = account.as_ref().map_or_else(
        || state.dummy_hash().clone(),
        |account| PasswordHash::unchecked_new(account.password_hash.clone()),
    );
    let permit = state.try_acquire_hash_slot().ok_or(AuthError::Limited)?;

    let valid = web::block(move || {
        let _permit = permit;
        stored_hash.verify(password.as_str())
    })
    .await
    .map_err(|_| AuthError::Unavailable)?;

    let Some(account) = account.filter(|_| valid) else {
        return Err(AuthError::InvalidLogin);
    };

    let token = SessionToken::generate();
    let hash = token.digest();
    let now = SystemTime::now();
    let expires_at = state.session_expiry(now).ok_or(AuthError::Unavailable)?;

    // Expired rows are removed in small batches on the request path, so no background job is needed.
    state.delete_expired_sessions(now).await;

    // A request still carrying an old cookie replaces that session instead of piling up rows.
    state
        .database()
        .repository::<AuthRepository>()
        .create_session(
            account.id,
            hash.as_bytes(),
            expires_at,
            session
                .map(|token| token.digest())
                .as_ref()
                .map(TokenHash::as_bytes),
        )
        .await
        .map_err(|_| AuthError::Unavailable)?;

    Ok(HttpResponse::Ok()
        .no_store()
        .cookie(state.session_cookie(&token))
        .json(account.identity()))
}

#[post("/logout")]
async fn logout(
    state: Data<ApiState>,
    session: Option<SessionToken>,
) -> Result<HttpResponse, AuthError> {
    if let Some(token) = session {
        state
            .database()
            .repository::<AuthRepository>()
            .revoke(token.digest().as_bytes())
            .await
            .map_err(|_| AuthError::Unavailable)?;
    }

    Ok(HttpResponse::NoContent()
        .no_store()
        .cookie(state.removal_cookie())
        .finish())
}

#[post("/renew")]
async fn renew(state: Data<ApiState>, session: SessionToken) -> Result<HttpResponse, AuthError> {
    let now = SystemTime::now();
    let expires_at = state.session_expiry(now).ok_or(AuthError::Unavailable)?;

    state
        .database()
        .repository::<AuthRepository>()
        .renew(session.digest().as_bytes(), now, expires_at)
        .await
        .map_err(|_| AuthError::Unavailable)?
        .ok_or(AuthError::Unauthorized)?;

    state.delete_expired_sessions(now).await;

    Ok(HttpResponse::NoContent()
        .no_store()
        .cookie(state.session_cookie(&session))
        .finish())
}

#[cfg(test)]
mod tests {
    use std::iter;

    use actix_web::{App, test};
    use sqlx::PgPool;

    use super::*;
    use crate::{
        api::configure_routes,
        tests::{self, app_state, upload_resources},
    };

    const ORIGIN: &str = "http://localhost:5173";

    #[sqlx::test(migrations = "../database/migrations")]
    async fn login_cookie_logout_and_copied_token_denial(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/setup")
            .insert_header(("Origin", ORIGIN))
            .set_json(
                serde_json::json!({"email":" Person@Example.test ","password":"a long test password"}),
            )
            .to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);

        let cookie = response
            .response()
            .cookies()
            .find(|c| c.name() == "pp_session")
            .unwrap()
            .into_owned();

        assert_eq!(cookie.http_only(), Some(true));
        assert!(!cookie.secure().unwrap_or_default());
        assert_eq!(cookie.path(), Some("/"));
        assert!(cookie.max_age().unwrap().whole_seconds() > 0);

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/renew")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let renewed_cookie = response
            .response()
            .cookies()
            .find(|c| c.name() == "pp_session")
            .unwrap();

        assert!(!renewed_cookie.secure().unwrap_or_default());

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let logout_cookie = response
            .response()
            .cookies()
            .find(|c| c.name() == "pp_session")
            .unwrap();

        assert!(!logout_cookie.secure().unwrap_or_default());

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/renew")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie)
            .to_request();

        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::UNAUTHORIZED
        );

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", ORIGIN))
            .set_json(
                serde_json::json!({"email":"person@example.test","password":"a long test password"}),
            )
            .to_request();

        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::OK
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn secure_cookie_setting_applies_to_issue_and_removal(pool: PgPool) {
        let (storage, upload_config, _directory) = upload_resources().await;
        let state = tests::state_with_upload_secure_cookie(pool, storage, upload_config, true);
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        let cookie = tests::sign_up(&app).await;

        assert_eq!(cookie.secure(), Some(true));

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie)
            .to_request();
        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let removal_cookie = response
            .response()
            .cookies()
            .find(|c| c.name() == "pp_session")
            .unwrap();

        assert_eq!(removal_cookie.secure(), Some(true));
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn origin_and_missing_session_fail_closed(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        for origin in [None, Some("null"), Some("http://evil.test")] {
            let mut req = test::TestRequest::post().uri("/api/v1/auth/logout");

            if let Some(value) = origin {
                req = req.insert_header(("Origin", value));
            }

            assert_eq!(
                test::call_service(&app, req.to_request()).await.status(),
                StatusCode::FORBIDDEN
            );
        }

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/renew")
            .insert_header(("Origin", ORIGIN))
            .to_request();

        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn passive_status_does_not_renew_and_expired_sessions_fail(pool: PgPool) {
        let (state, _directory) = app_state(pool.clone()).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/setup")
            .insert_header(("Origin", ORIGIN))
            .set_json(
                serde_json::json!({"email":"a@example.test","password":"a long test password"}),
            )
            .to_request();
        let res = test::call_service(&app, req).await;
        let cookie = res.response().cookies().next().unwrap().into_owned();
        let before: f64 =
            sqlx::query_scalar("SELECT extract(epoch FROM expires_at)::float8 FROM sessions")
                .fetch_one(&pool)
                .await
                .unwrap();

        let req = test::TestRequest::get()
            .uri("/api/v1/auth/status")
            .cookie(cookie.clone())
            .to_request();
        let res = test::call_service(&app, req).await;

        assert_eq!(res.headers().get("Cache-Control").unwrap(), "no-store");
        assert!(res.response().cookies().next().is_none());

        let json: serde_json::Value = test::read_body_json(res).await;

        assert!(json["identity"].is_object());
        assert_eq!(json["idle_timeout_ms"], 3_600_000);
        assert!(!json.to_string().contains(cookie.value()));

        let after: f64 =
            sqlx::query_scalar("SELECT extract(epoch FROM expires_at)::float8 FROM sessions")
                .fetch_one(&pool)
                .await
                .unwrap();

        assert_eq!(before, after);

        sqlx::query("UPDATE sessions SET expires_at=now()-interval '1 second'")
            .execute(&pool)
            .await
            .unwrap();

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/renew")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie)
            .to_request();

        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::UNAUTHORIZED
        );

        pool.close().await;

        let req = test::TestRequest::get()
            .uri("/api/v1/auth/status")
            .to_request();

        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn requests_have_bounded_bodies_and_attempts(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", ORIGIN))
            .insert_header(("Content-Type", "text/plain"))
            .set_payload("secret")
            .to_request();

        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", ORIGIN))
            .set_json(serde_json::json!({"email":"a@example.test","password":"x".repeat(5000)}))
            .to_request();

        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::PAYLOAD_TOO_LARGE
        );

        for attempt in 0..11 {
            let req = test::TestRequest::post()
                .uri("/api/v1/auth/login")
                .insert_header(("Origin", ORIGIN))
                .set_json(serde_json::json!({"email":"invalid","password":"short"}))
                .to_request();
            let expected = if attempt < 8 {
                StatusCode::UNPROCESSABLE_ENTITY
            } else {
                StatusCode::TOO_MANY_REQUESTS
            };

            assert_eq!(test::call_service(&app, req).await.status(), expected);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn other_routes_do_not_use_the_attempt_limit(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;

        for _ in 0..11 {
            let req = test::TestRequest::post()
                .uri("/api/v1/auth/renew")
                .insert_header(("Origin", ORIGIN))
                .to_request();

            assert_eq!(
                test::call_service(&app, req).await.status(),
                StatusCode::UNAUTHORIZED
            );
        }

        let req = test::TestRequest::post()
            .uri("/api/v1/auth/login")
            .insert_header(("Origin", ORIGIN))
            .set_json(
                serde_json::json!({"email":"a@example.test","password":"wrong horse battery"}),
            )
            .to_request();

        assert_eq!(
            test::call_service(&app, req).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn wrong_logins_are_limited_per_peer(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let wrong = serde_json::json!({"email":"a@example.test","password":"wrong horse battery"});

        for attempt in 0..11 {
            let req = test::TestRequest::post()
                .uri("/api/v1/auth/login")
                .insert_header(("Origin", ORIGIN))
                .set_json(&wrong)
                .to_request();
            let expected = if attempt < 10 {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::TOO_MANY_REQUESTS
            };

            assert_eq!(test::call_service(&app, req).await.status(), expected);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn successful_logins_do_not_use_the_attempt_limit(pool: PgPool) {
        let (state, _directory) = app_state(pool).await;
        let app = test::init_service(
            App::new()
                .app_data(Data::new(state))
                .configure(configure_routes),
        )
        .await;
        let credentials =
            serde_json::json!({"email":"a@example.test","password":"correct horse battery"});

        for action in iter::once("setup").chain(iter::repeat_n("login", 11)) {
            let req = test::TestRequest::post()
                .uri(&format!("/api/v1/auth/{action}"))
                .insert_header(("Origin", ORIGIN))
                .set_json(&credentials)
                .to_request();

            assert_eq!(test::call_service(&app, req).await.status(), StatusCode::OK);
        }
    }
}
