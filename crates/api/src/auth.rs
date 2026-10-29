use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use actix_web::{
    HttpRequest, HttpResponse, ResponseError,
    cookie::{Cookie, SameSite},
    error::JsonPayloadError,
    http::{StatusCode, header},
    web,
};
use email_address::EmailAddress;
use pp_core::{
    auth::Identity,
    config::{AuthConfig, SessionConfig},
};
use pp_database::auth::AuthStore;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::sync::Semaphore;

mod credentials;

const COOKIE_NAME: &str = "pp_session";

pub struct AuthState {
    store: AuthStore,
    origin: String,
    idle_timeout: Duration,
    hash_slots: Arc<Semaphore>,
    attempts: Mutex<HashMap<IpAddr, (Instant, u32)>>,
    dummy_hash: String,
}

impl AuthState {
    pub fn new(store: AuthStore, session: SessionConfig, auth: AuthConfig) -> Result<Self, String> {
        session.validate()?;
        auth.validate()?;

        let delta = time::Duration::try_from(session.idle_timeout)
            .map_err(|_| "session timeout overflow")?;

        OffsetDateTime::now_utc()
            .checked_add(delta)
            .ok_or("session timeout exceeds timestamp range")?;

        Ok(Self {
            store,
            origin: auth.app_origin,
            idle_timeout: session.idle_timeout,
            hash_slots: Arc::new(Semaphore::new(2)),
            attempts: Mutex::new(HashMap::new()),
            dummy_hash: credentials::hash_password("unused dummy credential")
                .map_err(|_| "password hashing unavailable")?,
        })
    }

    fn check_origin(&self, request: &HttpRequest) -> Result<(), AuthError> {
        if request
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            != Some(self.origin.as_str())
        {
            return Err(AuthError(
                StatusCode::FORBIDDEN,
                "request origin is not allowed",
            ));
        }

        Ok(())
    }

    fn peer_ip(request: &HttpRequest) -> IpAddr {
        request
            .peer_addr()
            .map(|addr| addr.ip())
            .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
    }

    fn allow_attempt(&self, request: &HttpRequest) -> Result<(), AuthError> {
        let peer = Self::peer_ip(request);
        let mut attempts = self.attempts.lock().map_err(|_| unavailable())?;
        let now = Instant::now();

        attempts.retain(|_, (start, _)| now.duration_since(*start) < Duration::from_secs(60));

        let failures = attempts.get(&peer).map_or(0, |(_, count)| *count);

        if failures >= 10 || (attempts.len() >= 1024 && !attempts.contains_key(&peer)) {
            return Err(limited());
        }

        Ok(())
    }

    /// Only failed credential attempts count, so successful sign-ins never lock anyone out.
    fn record_failure(&self, request: &HttpRequest) {
        if let Ok(mut attempts) = self.attempts.lock() {
            attempts
                .entry(Self::peer_ip(request))
                .or_insert((Instant::now(), 0))
                .1 += 1;
        }
    }

    fn expires_at(&self, now: OffsetDateTime) -> Result<OffsetDateTime, AuthError> {
        let delta = time::Duration::try_from(self.idle_timeout).map_err(|_| unavailable())?;
        now.checked_add(delta).ok_or_else(unavailable)
    }

    fn cookie(&self, token: String) -> Result<Cookie<'static>, AuthError> {
        let max_age = time::Duration::try_from(self.idle_timeout).map_err(|_| unavailable())?;
        Ok(Cookie::build(COOKIE_NAME, token)
            .path("/")
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(false)
            .max_age(max_age)
            .finish())
    }
}

#[derive(Debug)]
struct AuthError(StatusCode, &'static str);

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.1)
    }
}

impl ResponseError for AuthError {
    fn status_code(&self) -> StatusCode {
        self.0
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.0)
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .json(serde_json::json!({"error":self.1}))
    }
}

fn unavailable() -> AuthError {
    AuthError(
        StatusCode::SERVICE_UNAVAILABLE,
        "authentication service unavailable",
    )
}

fn limited() -> AuthError {
    AuthError(
        StatusCode::TOO_MANY_REQUESTS,
        "too many sign-in attempts; try again later",
    )
}

fn unauthorized() -> AuthError {
    AuthError(
        StatusCode::UNAUTHORIZED,
        "session expired or sign-in required",
    )
}

fn request_hash(request: &HttpRequest) -> Option<[u8; 32]> {
    credentials::token_hash(request.cookie(COOKIE_NAME)?.value())
}

#[derive(Deserialize)]
struct Credentials {
    email: String,
    password: String,
}

impl Credentials {
    fn validate(&self) -> Result<String, AuthError> {
        let email = self.email.trim().to_lowercase();

        if email.len() > 254
            || !EmailAddress::is_valid(&email)
            || !(12..=128).contains(&self.password.chars().count())
        {
            return Err(AuthError(
                StatusCode::BAD_REQUEST,
                "enter a valid email and a password of 12–128 characters",
            ));
        }

        Ok(email)
    }
}

#[derive(Serialize)]
struct AuthStatus {
    setup_available: bool,
    identity: Option<Identity>,
    idle_timeout_ms: u64,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .app_data(
                web::JsonConfig::default()
                    .limit(4096)
                    .error_handler(|error, _| {
                        let status = match error {
                            JsonPayloadError::Overflow { .. }
                            | JsonPayloadError::OverflowKnownLength { .. } => {
                                StatusCode::PAYLOAD_TOO_LARGE
                            }
                            JsonPayloadError::ContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
                            _ => StatusCode::BAD_REQUEST,
                        };
                        AuthError(status, "invalid credential request").into()
                    }),
            )
            .route("/status", web::get().to(status))
            .route("/setup", web::post().to(setup))
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/renew", web::post().to(renew)),
    );
}

async fn status(
    state: web::Data<AuthState>,
    request: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    let setup_available = state
        .store
        .setup_available()
        .await
        .map_err(|_| unavailable())?;

    let identity = match request_hash(&request) {
        Some(hash) => state
            .store
            .authenticate(&hash, OffsetDateTime::now_utc())
            .await
            .map_err(|_| unavailable())?,
        None => None,
    };

    let idle_timeout_ms =
        u64::try_from(state.idle_timeout.as_millis()).map_err(|_| unavailable())?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(AuthStatus {
            setup_available,
            identity,
            idle_timeout_ms,
        }))
}

async fn setup(
    state: web::Data<AuthState>,
    request: HttpRequest,
    input: web::Json<Credentials>,
) -> Result<HttpResponse, AuthError> {
    state.check_origin(&request)?;
    state.allow_attempt(&request)?;

    let email = input
        .validate()
        .inspect_err(|_| state.record_failure(&request))?;
    let permit = state
        .hash_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| limited())?;
    let password = input.into_inner().password;

    let password_hash = web::block(move || {
        let _permit = permit;
        credentials::hash_password(&password)
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;

    let (token, hash) = credentials::new_token();
    let now = OffsetDateTime::now_utc();
    let id = state
        .store
        .create_account(&email, &password_hash, &hash, state.expires_at(now)?)
        .await
        .map_err(|_| unavailable())?
        .ok_or(AuthError(
            StatusCode::CONFLICT,
            "initial setup is already complete",
        ))?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(state.cookie(token)?)
        .json(id))
}

async fn login(
    state: web::Data<AuthState>,
    request: HttpRequest,
    input: web::Json<Credentials>,
) -> Result<HttpResponse, AuthError> {
    state.check_origin(&request)?;
    state.allow_attempt(&request)?;

    let email = input
        .validate()
        .inspect_err(|_| state.record_failure(&request))?;
    let account = state
        .store
        .account_by_email(&email)
        .await
        .map_err(|_| unavailable())?;
    let encoded = account
        .as_ref()
        .map(|a| a.password_hash.clone())
        .unwrap_or_else(|| state.dummy_hash.clone());
    let password = input.into_inner().password;
    let permit = state
        .hash_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| limited())?;

    let valid = web::block(move || {
        let _permit = permit;
        credentials::verify_password(&password, &encoded)
    })
    .await
    .map_err(|_| unavailable())?;

    let Some(account) = account.filter(|_| valid) else {
        state.record_failure(&request);

        return Err(AuthError(
            StatusCode::UNAUTHORIZED,
            "invalid email or password",
        ));
    };

    let (token, hash) = credentials::new_token();
    let now = OffsetDateTime::now_utc();

    state
        .store
        .cleanup_expired(now, 100)
        .await
        .map_err(|_| unavailable())?;
    state
        .store
        .create_session(
            account.id,
            &hash,
            state.expires_at(now)?,
            request_hash(&request).as_ref(),
        )
        .await
        .map_err(|_| unavailable())?;

    let id = state
        .store
        .authenticate(&hash, now)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;

    Ok(HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(state.cookie(token)?)
        .json(id))
}

async fn logout(
    state: web::Data<AuthState>,
    request: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    state.check_origin(&request)?;

    if let Some(hash) = request_hash(&request) {
        state.store.revoke(&hash).await.map_err(|_| unavailable())?;
    }

    let mut cookie = state.cookie(String::new())?;
    cookie.make_removal();

    Ok(HttpResponse::NoContent()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(cookie)
        .finish())
}

async fn renew(
    state: web::Data<AuthState>,
    request: HttpRequest,
) -> Result<HttpResponse, AuthError> {
    state.check_origin(&request)?;
    let hash = request_hash(&request).ok_or_else(unauthorized)?;
    let now = OffsetDateTime::now_utc();

    state
        .store
        .renew(&hash, now, state.expires_at(now)?)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unauthorized)?;
    state
        .store
        .cleanup_expired(now, 100)
        .await
        .map_err(|_| unavailable())?;
    let token = request
        .cookie(COOKIE_NAME)
        .ok_or_else(unauthorized)?
        .value()
        .to_owned();

    Ok(HttpResponse::NoContent()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .cookie(state.cookie(token)?)
        .finish())
}
