//! Gallery HTTP API: route wiring, process-wide authentication state and the
//! [`server::Api`] runner.

use std::{net::IpAddr, sync::Arc, time::Duration};

use actix_web::{
    cookie::{Cookie, SameSite},
    web,
};
use derive_more::{Display, Error, From};
use pp_core::{
    config::{AuthConfig, SessionConfig},
    time::CookieDuration,
    url::Origin,
};
use pp_database::{Database, auth::AuthRepository};
use time::{Duration as TimeDuration, OffsetDateTime, error::ConversionRange};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::{
    credentials::{HashError, PasswordHash, SessionToken},
    limiter::AttemptLimiter,
};

pub mod credentials;
pub mod routes;
pub mod server;

mod limiter;

/// Argon2 is memory heavy, so only this many hashes run at once; further requests are refused
/// instead of queued.
const HASH_SLOTS: usize = 2;

/// Startup failure while building [`AppState`].
#[derive(Debug, Display, Error, From)]
pub enum StateError {
    #[display("session idle timeout is out of range")]
    SessionLifetime(ConversionRange),
    #[display("password hashing unavailable")]
    Hashing(HashError),
}

/// Process-wide state shared by every request handler.
///
/// Built once at startup from validated configuration, so request handlers never re-check
/// configuration and cannot fail for configuration reasons. It holds the authentication
/// policy, storage and abuse controls.
pub(crate) struct AppState {
    database: Database,
    origin: Origin,
    idle_timeout: CookieDuration,
    // The same span as `idle_timeout` in the `time` crate's type, converted once at startup.
    session_lifetime: TimeDuration,
    hash_slots: Arc<Semaphore>,
    failed_attempts: AttemptLimiter,
    dummy_hash: PasswordHash,
}

impl AppState {
    pub fn new(
        database: Database,
        session: SessionConfig,
        auth: AuthConfig,
    ) -> Result<Self, StateError> {
        Ok(Self {
            database,
            origin: auth.app_origin,
            idle_timeout: session.idle_timeout,
            // `CookieDuration` bounds the timeout far below the `time` and timestamp limits,
            // so adding it to the current time cannot overflow for centuries.
            session_lifetime: TimeDuration::try_from(session.idle_timeout.get())?,
            hash_slots: Arc::new(Semaphore::new(HASH_SLOTS)),
            failed_attempts: AttemptLimiter::default(),
            dummy_hash: PasswordHash::generate()?,
        })
    }

    pub fn store(&self) -> AuthRepository<'_> {
        self.database.repository()
    }

    /// True only for the exact configured origin; a missing header is not allowed.
    pub fn allows_origin(&self, header: Option<&str>) -> bool {
        header == Some(self.origin.as_str())
    }

    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout.get()
    }

    /// Expiry of a session created or renewed at `now`; `None` only if it overflows time.
    pub fn session_expiry(&self, now: OffsetDateTime) -> Option<OffsetDateTime> {
        now.checked_add(self.session_lifetime)
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
    pub async fn delete_expired_sessions(&self, now: OffsetDateTime) {
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
        Cookie::build(SessionToken::COOKIE_NAME, value.to_owned())
            .path("/")
            .http_only(true)
            .same_site(SameSite::Lax)
            .secure(false)
            .max_age(self.session_lifetime)
            .finish()
    }
}

/// Mounts every HTTP route under `/api/v1`.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/v1")
            .service(routes::health::report_health)
            .configure(routes::auth::configure),
    );
}
