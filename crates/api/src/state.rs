//! Process-wide state shared by every request handler.
//!
//! Built once at startup from validated configuration, so request handlers never re-check
//! configuration and cannot fail for configuration reasons.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

use actix_web::cookie::{Cookie, SameSite};
use derive_more::{Display, Error, From};
use pp_core::{
    config::{AuthConfig, SessionConfig},
    time::NonZeroDuration,
    url::Origin,
};
use pp_database::{Database, auth::AuthStore};
use time::{OffsetDateTime, error::ConversionRange};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::credentials::{HashError, PasswordHash, SessionToken};

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

/// Authentication policy, storage and abuse controls.
pub struct AppState {
    database: Database,
    origin: Origin,
    idle_timeout: NonZeroDuration,
    // The same span as `idle_timeout` in the `time` crate's type, converted once at startup.
    session_lifetime: time::Duration,
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
            // `NonZeroDuration` bounds the timeout far below the `time` and timestamp limits,
            // so adding it to the current time cannot overflow for centuries.
            session_lifetime: time::Duration::try_from(session.idle_timeout.get())?,
            hash_slots: Arc::new(Semaphore::new(HASH_SLOTS)),
            failed_attempts: AttemptLimiter::default(),
            dummy_hash: PasswordHash::new("unused dummy credential")?,
        })
    }

    pub fn store(&self) -> AuthStore<'_> {
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

/// Fixed-window limiter of failed sign-ins per peer: 10 failures per minute.
///
/// State is in-process, bounded to 1024 peers, and resets on restart. Behind a proxy every
/// client shares the proxy's address, so they share one allowance.
#[derive(Default)]
struct AttemptLimiter {
    windows: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl AttemptLimiter {
    const WINDOW: Duration = Duration::from_secs(60);
    const MAX_FAILURES: u32 = 10;
    const MAX_PEERS: usize = 1024;

    fn allows(&self, peer: IpAddr) -> bool {
        let mut windows = self.lock();
        let now = Instant::now();

        windows.retain(|_, (start, _)| now.duration_since(*start) < Self::WINDOW);

        let failures = windows.get(&peer).map_or(0, |(_, count)| *count);

        failures < Self::MAX_FAILURES
            && (windows.len() < Self::MAX_PEERS || windows.contains_key(&peer))
    }

    fn record(&self, peer: IpAddr) {
        self.lock().entry(peer).or_insert((Instant::now(), 0)).1 += 1;
    }

    // The map holds plain counters, so state left by a panicking holder is still valid.
    fn lock(&self) -> MutexGuard<'_, HashMap<IpAddr, (Instant, u32)>> {
        self.windows.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
