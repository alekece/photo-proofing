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
//! configured `Origin`, which [`require_same_origin`] enforces as the CSRF defence for this
//! cookie-authenticated API before any extractor reads the request. Sign-in routes also pass
//! the failed-attempt limiter ([`Attempt`]). Request bodies are validated while being parsed
//! ([`Credentials`]), and policy and limiter state live in [`ApiState`].

use std::time::SystemTime;

use actix_web::{
    Error, HttpResponse, ResponseError,
    body::{EitherBody, MessageBody},
    dev::{ServiceRequest, ServiceResponse},
    error::JsonPayloadError,
    get,
    http::{StatusCode, header},
    middleware::{Next, from_fn},
    post, web,
};
use pp_core::{email::Email, time::Millis};
use pp_database::auth::Identity;
use serde::{Deserialize, Serialize};
use serde_json::error::Category;

use crate::{
    api::ApiState,
    credentials::{Password, PasswordHash},
    extractors::{
        attempt::Attempt,
        session_token::{SessionToken, TokenHash},
    },
    request::RequestExt,
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
    #[error("enter a valid email and a password of 12–128 characters")]
    InvalidInput,
    #[error("invalid email or password")]
    InvalidLogin,
    #[error("initial setup is already complete")]
    SetupClosed,
    #[error("invalid credential request")]
    PayloadTooLarge,
    #[error("invalid credential request")]
    UnsupportedMediaType,
    #[error("invalid credential request")]
    MalformedRequest,
}

impl From<JsonPayloadError> for AuthError {
    fn from(error: JsonPayloadError) -> Self {
        match error {
            JsonPayloadError::Overflow { .. } | JsonPayloadError::OverflowKnownLength { .. } => {
                Self::PayloadTooLarge
            }
            JsonPayloadError::ContentType => Self::UnsupportedMediaType,
            // Well-formed JSON whose email or password failed validation while parsing.
            JsonPayloadError::Deserialize(error) if error.classify() == Category::Data => {
                Self::InvalidInput
            }
            _ => Self::MalformedRequest,
        }
    }
}

impl ResponseError for AuthError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::Limited => StatusCode::TOO_MANY_REQUESTS,
            Self::Unauthorized | Self::InvalidLogin => StatusCode::UNAUTHORIZED,
            Self::ForbiddenOrigin => StatusCode::FORBIDDEN,
            Self::InvalidInput | Self::MalformedRequest => StatusCode::BAD_REQUEST,
            Self::SetupClosed => StatusCode::CONFLICT,
            Self::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .insert_header((header::CACHE_CONTROL, "no-store"))
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

/// Rejects state-changing requests whose `Origin` is not the configured one, before their
/// payload is read.
async fn require_same_origin<B: MessageBody + 'static>(
    request: ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<EitherBody<B>>, Error> {
    let allowed = request.method().is_safe()
        || request
            .app_data::<web::Data<ApiState>>()
            .is_some_and(|state| {
                let origin = request
                    .headers()
                    .get(header::ORIGIN)
                    .and_then(|value| value.to_str().ok());

                state.allows_origin(origin)
            });

    if !allowed {
        return Ok(request
            .error_response(AuthError::ForbiddenOrigin)
            .map_into_right_body());
    }

    Ok(next.call(request).await?.map_into_left_body())
}

/// Registers the auth routes; credential bodies are capped at 4 KiB.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .wrap(from_fn(require_same_origin))
            .app_data(
                web::JsonConfig::default()
                    .limit(4096)
                    .error_handler(|error, request| {
                        // Only bodies that parse but fail validation count as guesses; size and
                        // content-type rejections are cheap and never reach credential checks.
                        if matches!(error, JsonPayloadError::Deserialize(_))
                            && let Some(state) = request.app_data::<web::Data<ApiState>>()
                        {
                            state.record_failure(request.peer_ip());
                        }

                        AuthError::from(error).into()
                    }),
            )
            .service(report_status)
            .service(setup)
            .service(login)
            .service(logout)
            .service(renew),
    );
}

#[get("/status")]
async fn report_status(
    state: web::Data<ApiState>,
    session: Option<SessionToken>,
) -> Result<HttpResponse, AuthError> {
    let setup_available = state
        .store()
        .setup_available()
        .await
        .map_err(|_| AuthError::Unavailable)?;

    let identity = match session {
        Some(token) => state
            .store()
            .authenticate(token.digest().as_bytes(), SystemTime::now())
            .await
            .map_err(|_| AuthError::Unavailable)?,
        None => None,
    };

    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(AuthStatus {
            setup_available,
            identity,
            idle_timeout: state.idle_timeout().into(),
        }))
}

#[post("/setup")]
async fn setup(
    state: web::Data<ApiState>,
    _attempt: Attempt,
    credentials: web::Json<Credentials>,
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
        .store()
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
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(state.session_cookie(&token))
        .json(id))
}

#[post("/login")]
async fn login(
    state: web::Data<ApiState>,
    session: Option<SessionToken>,
    attempt: Attempt,
    credentials: web::Json<Credentials>,
) -> Result<HttpResponse, AuthError> {
    let Credentials { email, password } = credentials.into_inner();
    let account = state
        .store()
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
        state.record_failure(attempt.peer());

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
        .store()
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
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(state.session_cookie(&token))
        .json(account.identity()))
}

#[post("/logout")]
async fn logout(
    state: web::Data<ApiState>,
    session: Option<SessionToken>,
) -> Result<HttpResponse, AuthError> {
    if let Some(token) = session {
        state
            .store()
            .revoke(token.digest().as_bytes())
            .await
            .map_err(|_| AuthError::Unavailable)?;
    }

    Ok(HttpResponse::NoContent()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(state.removal_cookie())
        .finish())
}

#[post("/renew")]
async fn renew(
    state: web::Data<ApiState>,
    session: SessionToken,
) -> Result<HttpResponse, AuthError> {
    let now = SystemTime::now();
    let expires_at = state.session_expiry(now).ok_or(AuthError::Unavailable)?;

    state
        .store()
        .renew(session.digest().as_bytes(), now, expires_at)
        .await
        .map_err(|_| AuthError::Unavailable)?
        .ok_or(AuthError::Unauthorized)?;

    state.delete_expired_sessions(now).await;

    Ok(HttpResponse::NoContent()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(state.session_cookie(&session))
        .finish())
}

#[cfg(test)]
mod tests {
    use std::{
        iter,
        num::{NonZeroU32, NonZeroUsize},
    };

    use actix_web::{App, test};
    use pp_database::Database;
    use sqlx::PgPool;

    use super::*;
    use crate::{
        api::configure_routes,
        config::{AuthConfig, LimiterConfig, SessionConfig},
    };

    const ORIGIN: &str = "http://localhost:5173";

    fn app_state(pool: PgPool) -> ApiState {
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

    #[sqlx::test(migrations = "../database/migrations")]
    async fn login_cookie_logout_and_copied_token_denial(pool: PgPool) {
        let state = app_state(pool);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
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
        assert_eq!(cookie.path(), Some("/"));
        assert!(cookie.max_age().unwrap().whole_seconds() > 0);

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/renew")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .to_request();

        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::NO_CONTENT
        );

        let request = test::TestRequest::post()
            .uri("/api/v1/auth/logout")
            .insert_header(("Origin", ORIGIN))
            .cookie(cookie.clone())
            .to_request();

        assert_eq!(
            test::call_service(&app, request).await.status(),
            StatusCode::NO_CONTENT
        );

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
    async fn origin_and_missing_session_fail_closed(pool: PgPool) {
        let state = app_state(pool);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
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
        let state = app_state(pool.clone());
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
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
        let state = app_state(pool);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
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
            let expected = if attempt < 10 {
                StatusCode::BAD_REQUEST
            } else {
                StatusCode::TOO_MANY_REQUESTS
            };

            assert_eq!(test::call_service(&app, req).await.status(), expected);
        }
    }

    #[sqlx::test(migrations = "../database/migrations")]
    async fn successful_logins_do_not_use_the_attempt_limit(pool: PgPool) {
        let state = app_state(pool);
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
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
