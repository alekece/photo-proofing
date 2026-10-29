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
//! configured `Origin`, which is the CSRF defence for this cookie-authenticated API. Policy
//! and limiter state live in [`AppState`].

use std::{
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

use actix_web::{
    HttpRequest, HttpResponse, ResponseError,
    cookie::{Cookie, SameSite},
    error::JsonPayloadError,
    http::{StatusCode, header},
    web,
};
use derive_more::{Display, Error};
use email_address::EmailAddress;
use pp_core::auth::Identity;
use serde::{Deserialize, Serialize, Serializer, ser::Error as _};
use time::OffsetDateTime;

use crate::{
    credentials::{PasswordHash, SessionToken, TokenHash},
    state::AppState,
};

const COOKIE_NAME: &str = "pp_session";

/// A safe, client-facing failure: the status and a fixed message that never echoes input or
/// internal details. Rendered as `{"error": message}` with `Cache-Control: no-store`.
#[derive(Debug, Display, Error)]
#[display("{message}")]
struct AuthError {
    status: StatusCode,
    message: &'static str,
}

impl AuthError {
    const fn new(status: StatusCode, message: &'static str) -> Self {
        Self { status, message }
    }

    const fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "authentication service unavailable",
        )
    }

    const fn limited() -> Self {
        Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "too many sign-in attempts; try again later",
        )
    }

    const fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "session expired or sign-in required",
        )
    }

    const fn forbidden_origin() -> Self {
        Self::new(StatusCode::FORBIDDEN, "request origin is not allowed")
    }

    const fn invalid_input() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "enter a valid email and a password of 12–128 characters",
        )
    }

    const fn invalid_login() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "invalid email or password")
    }

    const fn setup_closed() -> Self {
        Self::new(StatusCode::CONFLICT, "initial setup is already complete")
    }
}

impl From<JsonPayloadError> for AuthError {
    fn from(error: JsonPayloadError) -> Self {
        let status = match error {
            JsonPayloadError::Overflow { .. } | JsonPayloadError::OverflowKnownLength { .. } => {
                StatusCode::PAYLOAD_TOO_LARGE
            }
            JsonPayloadError::ContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            _ => StatusCode::BAD_REQUEST,
        };

        Self::new(status, "invalid credential request")
    }
}

impl ResponseError for AuthError {
    fn status_code(&self) -> StatusCode {
        self.status
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status)
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(serde_json::json!({"error": self.message}))
    }
}

/// Request body as sent by the browser; becomes a [`SignIn`] once it passes validation.
#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
}

/// Credentials with a normalized (trimmed, lowercased) email and a password within bounds.
struct SignIn {
    email: String,
    password: String,
}

impl TryFrom<Credentials> for SignIn {
    type Error = AuthError;

    fn try_from(credentials: Credentials) -> Result<Self, Self::Error> {
        let email = credentials.email.trim().to_lowercase();

        if email.len() > 254
            || !EmailAddress::is_valid(&email)
            || !(12..=128).contains(&credentials.password.chars().count())
        {
            return Err(AuthError::invalid_input());
        }

        Ok(Self {
            email,
            password: credentials.password,
        })
    }
}

#[derive(Serialize)]
struct AuthStatus {
    setup_available: bool,
    identity: Option<Identity>,
    #[serde(rename = "idle_timeout_ms", serialize_with = "serialize_millis")]
    idle_timeout: Duration,
}

/// The web app consumes the timeout as integer milliseconds, so convert only on the wire.
fn serialize_millis<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    let millis = u64::try_from(duration.as_millis()).map_err(S::Error::custom)?;

    serializer.serialize_u64(millis)
}

/// Registers the auth routes; credential bodies are capped at 4 KiB.
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .app_data(
                web::JsonConfig::default()
                    .limit(4096)
                    .error_handler(|error, _| AuthError::from(error).into()),
            )
            .route("/status", web::get().to(status))
            .route("/setup", web::post().to(setup))
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/renew", web::post().to(renew)),
    );
}

async fn status(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    let setup_available = state
        .store()
        .setup_available()
        .await
        .map_err(|_| AuthError::unavailable())?;

    let identity = match session_hash(&request) {
        Some(hash) => state
            .store()
            .authenticate(hash.as_bytes(), OffsetDateTime::now_utc())
            .await
            .map_err(|_| AuthError::unavailable())?,
        None => None,
    };

    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(AuthStatus {
            setup_available,
            identity,
            idle_timeout: state.idle_timeout(),
        }))
}

async fn setup(
    state: web::Data<AppState>,
    request: HttpRequest,
    input: web::Json<Credentials>,
) -> Result<HttpResponse, AuthError> {
    check_origin(&state, &request)?;
    check_attempt_allowed(&state, &request)?;

    let SignIn { email, password } = SignIn::try_from(input.into_inner())
        .inspect_err(|_| state.record_failure(peer_ip(&request)))?;
    let permit = state
        .try_acquire_hash_slot()
        .ok_or_else(AuthError::limited)?;

    let password_hash = web::block(move || {
        let _permit = permit;
        PasswordHash::new(&password)
    })
    .await
    .map_err(|_| AuthError::unavailable())?
    .map_err(|_| AuthError::unavailable())?;

    // Concurrent setups can both reach here; the database allows only one account and
    // `create_account` returns `None` to the loser, which becomes the 409 below.
    let token = SessionToken::generate();
    let now = OffsetDateTime::now_utc();
    let id = state
        .store()
        .create_account(
            &email,
            password_hash.as_str(),
            token.digest().as_bytes(),
            session_expiry(&state, now)?,
        )
        .await
        .map_err(|_| AuthError::unavailable())?
        .ok_or(AuthError::setup_closed())?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(session_cookie(&state, token.as_str()))
        .json(id))
}

async fn login(
    state: web::Data<AppState>,
    request: HttpRequest,
    input: web::Json<Credentials>,
) -> Result<HttpResponse, AuthError> {
    check_origin(&state, &request)?;
    check_attempt_allowed(&state, &request)?;

    let SignIn { email, password } = SignIn::try_from(input.into_inner())
        .inspect_err(|_| state.record_failure(peer_ip(&request)))?;
    let account = state
        .store()
        .account_by_email(&email)
        .await
        .map_err(|_| AuthError::unavailable())?;
    // Verify against a dummy hash when the email is unknown so both cases cost the same.
    let stored_hash = account.as_ref().map_or_else(
        || state.dummy_hash().clone(),
        |account| PasswordHash::from_stored(account.password_hash.clone()),
    );
    let permit = state
        .try_acquire_hash_slot()
        .ok_or_else(AuthError::limited)?;

    let valid = web::block(move || {
        let _permit = permit;
        stored_hash.verify(&password)
    })
    .await
    .map_err(|_| AuthError::unavailable())?;

    let Some(account) = account.filter(|_| valid) else {
        state.record_failure(peer_ip(&request));

        return Err(AuthError::invalid_login());
    };

    let token = SessionToken::generate();
    let hash = token.digest();
    let now = OffsetDateTime::now_utc();

    // Expired rows are removed in small batches on the request path, so no background job is needed.
    state
        .store()
        .cleanup_expired(now, 100)
        .await
        .map_err(|_| AuthError::unavailable())?;
    // A request still carrying an old cookie replaces that session instead of piling up rows.
    state
        .store()
        .create_session(
            account.id,
            hash.as_bytes(),
            session_expiry(&state, now)?,
            session_hash(&request).as_ref().map(TokenHash::as_bytes),
        )
        .await
        .map_err(|_| AuthError::unavailable())?;

    let id = state
        .store()
        .authenticate(hash.as_bytes(), now)
        .await
        .map_err(|_| AuthError::unavailable())?
        .ok_or_else(AuthError::unavailable)?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(session_cookie(&state, token.as_str()))
        .json(id))
}

async fn logout(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    check_origin(&state, &request)?;

    if let Some(hash) = session_hash(&request) {
        state
            .store()
            .revoke(hash.as_bytes())
            .await
            .map_err(|_| AuthError::unavailable())?;
    }

    let mut cookie = session_cookie(&state, "");
    cookie.make_removal();

    Ok(HttpResponse::NoContent()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(cookie)
        .finish())
}

async fn renew(
    state: web::Data<AppState>,
    request: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    check_origin(&state, &request)?;

    let token = session_token(&request).ok_or_else(AuthError::unauthorized)?;
    let now = OffsetDateTime::now_utc();

    state
        .store()
        .renew(token.digest().as_bytes(), now, session_expiry(&state, now)?)
        .await
        .map_err(|_| AuthError::unavailable())?
        .ok_or_else(AuthError::unauthorized)?;
    state
        .store()
        .cleanup_expired(now, 100)
        .await
        .map_err(|_| AuthError::unavailable())?;

    Ok(HttpResponse::NoContent()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(session_cookie(&state, token.as_str()))
        .finish())
}

/// The cookie's token, or `None` when absent or malformed.
fn session_token(request: &HttpRequest) -> Option<SessionToken> {
    request.cookie(COOKIE_NAME)?.value().parse().ok()
}

fn session_hash(request: &HttpRequest) -> Option<TokenHash> {
    session_token(request).map(|token| token.digest())
}

fn check_origin(state: &AppState, request: &HttpRequest) -> Result<(), AuthError> {
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok());

    if origin != Some(state.origin().as_str()) {
        return Err(AuthError::forbidden_origin());
    }

    Ok(())
}

fn peer_ip(request: &HttpRequest) -> IpAddr {
    request
        .peer_addr()
        .map(|addr| addr.ip())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

fn check_attempt_allowed(state: &AppState, request: &HttpRequest) -> Result<(), AuthError> {
    if !state.allow_attempt(peer_ip(request)) {
        return Err(AuthError::limited());
    }

    Ok(())
}

fn session_expiry(state: &AppState, now: OffsetDateTime) -> Result<OffsetDateTime, AuthError> {
    state.session_expiry(now).ok_or_else(AuthError::unavailable)
}

/// `Secure` stays off because the trusted-LAN trial runs over plain HTTP (see ADR 0002).
fn session_cookie(state: &AppState, value: &str) -> Cookie<'static> {
    Cookie::build(COOKIE_NAME, value.to_owned())
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(false)
        .max_age(state.session_lifetime())
        .finish()
}
