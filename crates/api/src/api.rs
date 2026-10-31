use std::{
    io::Error as IoError,
    net::IpAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use actix_web::{
    App, HttpServer,
    cookie::{Cookie, SameSite, time::Duration as CookieMaxAge, time::error::ConversionRange},
    web::{self, Data, JsonConfig, PathConfig, QueryConfig, ServiceConfig},
};
use derive_more::From;
use pp_core::url::Origin;
use pp_database::{
    Database, auth::AuthRepository, error::DatabaseError, gallery::GalleryRepository,
};
use thiserror::Error;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    config::{
        ApiConfig, AuthConfig, CookieDuration, CookieName, HttpConfig, LimiterConfig, SessionConfig,
    },
    credentials::{HashError, PasswordHash},
    error::ErrorResponse,
    extractors::session_token::SessionToken,
    limiter::AttemptLimiter,
    middlewares::origin::SameOrigin,
    routes,
};

/// Largest accepted JSON body: credentials and gallery changesets are a handful of short
/// fields, so this is well above any valid request.
pub(crate) const JSON_LIMIT: usize = 4096;

/// Argon2 is memory heavy, so only this many hashes run at once; further requests are refused
/// instead of queued.
const HASH_SLOTS: usize = 2;

/// Failure to start or run the [`Api`].
///
/// Database causes are carried as sources because [`DatabaseError`] is sanitized: it never
/// contains the database URL, which holds credentials.
#[derive(Debug, Error, From)]
pub enum ApiError {
    #[error("database failed")]
    Database(#[source] DatabaseError),
    #[error("session idle timeout is out of range")]
    SessionLifetime(#[source] ConversionRange),
    #[error("password hashing unavailable")]
    Hashing(#[source] HashError),
    #[error("server failed")]
    Serve(#[source] IoError),
}

/// Process-wide state shared by every request handler.
///
/// Built once at startup from validated configuration, so request handlers never re-check
/// configuration and cannot fail for configuration reasons. It holds the authentication
/// policy, storage and abuse controls.
pub(crate) struct ApiState {
    database: Database,
    origin: Origin,
    idle_timeout: CookieDuration,
    cookie_name: CookieName,
    // The same span as `idle_timeout` in the cookie crate's type, converted once at startup.
    cookie_max_age: CookieMaxAge,
    hash_slots: Arc<Semaphore>,
    failed_attempts: AttemptLimiter,
    dummy_hash: PasswordHash,
}

impl ApiState {
    pub fn new(
        database: Database,
        session: SessionConfig,
        auth: AuthConfig,
        limiter: LimiterConfig,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            database,
            origin: auth.app_origin,
            idle_timeout: session.idle_timeout,
            cookie_name: session.cookie_name,
            // `CookieDuration` bounds the timeout far below the cookie and timestamp limits,
            // so adding it to the current time cannot overflow for centuries.
            cookie_max_age: CookieMaxAge::try_from(session.idle_timeout.get())?,
            hash_slots: Arc::new(Semaphore::new(HASH_SLOTS)),
            failed_attempts: AttemptLimiter::new(limiter),
            dummy_hash: PasswordHash::generate()?,
        })
    }

    pub fn store(&self) -> AuthRepository<'_> {
        self.database.repository()
    }

    pub fn galleries(&self) -> GalleryRepository<'_> {
        self.database.repository()
    }

    /// True only for the exact configured origin; a missing header is not allowed.
    pub fn allows_origin(&self, header: Option<&str>) -> bool {
        header == Some(self.origin.as_str())
    }

    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout.get()
    }

    /// Name of the cookie carrying the session token.
    pub fn cookie_name(&self) -> &str {
        self.cookie_name.as_str()
    }

    /// Expiry of a session created or renewed at `now`; `None` only if it overflows time.
    pub fn session_expiry(&self, now: SystemTime) -> Option<SystemTime> {
        now.checked_add(self.idle_timeout.get())
    }

    /// The session cookie for `token`, living as long as the idle timeout.
    ///
    /// `Secure` stays off because the trusted-LAN trial runs over plain HTTP (see ADR 0002).
    pub fn session_cookie(&self, token: &SessionToken) -> Cookie<'static> {
        self.cookie(token.as_str())
    }

    /// A cookie that makes the browser drop the session cookie.
    pub fn removal_cookie(&self) -> Cookie<'static> {
        let mut cookie = self.cookie("");

        cookie.make_removal();

        cookie
    }

    /// Best-effort removal of a small batch of expired sessions. They already fail
    /// authentication, so a failure here is logged and must never fail the caller's request.
    pub async fn delete_expired_sessions(&self, now: SystemTime) {
        if let Err(error) = self.store().cleanup_expired(now, 100).await {
            tracing::warn!(%error, "expired session cleanup failed");
        }
    }

    /// Hash verified for unknown emails so they cost as much as known ones.
    pub fn dummy_hash(&self) -> &PasswordHash {
        &self.dummy_hash
    }

    /// Reserves one of the bounded password-hashing slots until the permit is dropped.
    pub fn try_acquire_hash_slot(&self) -> Option<OwnedSemaphorePermit> {
        Arc::clone(&self.hash_slots).try_acquire_owned().ok()
    }

    /// False once the peer has failed too many times recently; see [`AttemptLimiter`].
    pub fn allow_attempt(&self, peer: IpAddr) -> bool {
        self.failed_attempts.allows(peer)
    }

    /// Only failed credential attempts count, so successful sign-ins never lock anyone out.
    pub fn record_failure(&self, peer: IpAddr) {
        self.failed_attempts.record(peer);
    }

    fn cookie(&self, value: &str) -> Cookie<'static> {
        Cookie::build(self.cookie_name.as_str().to_owned(), value.to_owned())
            .path("/")
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(false)
            .max_age(self.cookie_max_age)
            .finish()
    }
}

/// The API process: [`init`](Self::init) connects and prepares everything the server needs
/// (database, migrations, authentication state), so [`run`](Self::run) only serves requests.
/// A database or configuration problem therefore stops startup instead of opening setup on an
/// unknown state.
pub struct Api {
    http: HttpConfig,
    state: Data<ApiState>,
}

impl Api {
    pub async fn init(config: ApiConfig) -> Result<Self, ApiError> {
        let database = Database::connect_lazy(&config.database.url, 10)?;

        database.ensure_reachable().await?;
        database.migrate().await?;

        let state = ApiState::new(database, config.session, config.auth, config.limiter)?;

        Ok(Self {
            http: config.http,
            state: Data::new(state),
        })
    }

    /// Serves until the process is stopped.
    pub async fn run(self) -> Result<(), ApiError> {
        tracing::info!(host = %self.http.host, port = self.http.port, "api starting");

        let state = self.state;

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                .configure(configure_routes)
        })
        .bind((self.http.host.as_str(), self.http.port))?
        .run()
        .await?;

        Ok(())
    }
}

/// Mounts every HTTP route under `/api/v1`, behind the same-origin check, with the shared
/// request-parsing error responses.
pub fn configure_routes(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .wrap(SameOrigin)
            .app_data(
                JsonConfig::default()
                    .limit(JSON_LIMIT)
                    .error_handler(|error, _| ErrorResponse::from(error).into()),
            )
            .app_data(
                QueryConfig::default().error_handler(|_, _| ErrorResponse::bad_request().into()),
            )
            .app_data(PathConfig::default().error_handler(|_, _| ErrorResponse::not_found().into()))
            .service(routes::health::report_health)
            .configure(routes::auth::configure)
            .configure(routes::galleries::configure),
    );
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use pp_database::error::DatabaseError;
    use sqlx::Error as SqlxError;

    use super::*;

    #[test]
    fn database_errors_expose_a_sanitized_cause() {
        let error = ApiError::from(DatabaseError::from(SqlxError::Configuration(
            "unknown value \"s3cretpw\" for `ssl_mode`".into(),
        )));
        let cause = error.source().expect("the database cause is carried");

        assert!(!error.to_string().contains("s3cretpw"));
        assert!(!cause.to_string().contains("s3cretpw"));
        assert!(cause.source().is_none());
    }

    #[test]
    fn serve_error_exposes_its_cause_once() {
        let error = ApiError::from(IoError::other("bind failed"));

        assert_eq!(error.to_string(), "server failed");
        assert_eq!(error.source().unwrap().to_string(), "bind failed");
    }
}
