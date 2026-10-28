use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    fmt,
    future::Future,
    sync::{Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TOKEN_BYTES: usize = 32;
const TOKEN_ENCODED_LEN: usize = 43;

/// Opaque 256-bit bearer token handed to the client. Redacted in `Debug`,
/// zeroized on drop, and never persisted: only its [`SessionTokenHash`] is.
#[derive(Clone)]
pub struct SessionToken(SecretString);

impl SessionToken {
    pub fn generate() -> Self {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).expect("the operating system CSPRNG is available");

        Self(SecretString::from(URL_SAFE_NO_PAD.encode(bytes)))
    }

    /// Parses a client-supplied value, rejecting anything that is not a well-formed token
    /// before it reaches the store.
    pub fn parse(value: &str) -> Option<Self> {
        if value.len() != TOKEN_ENCODED_LEN {
            return None;
        }

        let bytes = URL_SAFE_NO_PAD.decode(value).ok()?;

        (bytes.len() == TOKEN_BYTES).then(|| Self(SecretString::from(value)))
    }

    /// The value to put in the cookie. Handle with care: never log it.
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }

    pub fn hash(&self) -> SessionTokenHash {
        SessionTokenHash(Sha256::digest(self.expose().as_bytes()).into())
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken([REDACTED])")
    }
}

/// SHA-256 of a token. A fast hash is sufficient because tokens carry 256 bits
/// of entropy, so there is nothing to brute-force; a leaked store cannot be
/// replayed as cookies.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SessionTokenHash([u8; 32]);

impl SessionTokenHash {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for SessionTokenHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionTokenHash([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub token_hash: SessionTokenHash,
    pub created_at: SystemTime,
    pub expires_at: SystemTime,
    pub last_rotated_at: SystemTime,
    pub revoked_at: Option<SystemTime>,
}

pub trait Clock: Send + Sync {
    fn now(&self) -> SystemTime;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

/// Persistence for sessions, keyed by token hash. Implementations must make
/// `revoke` and `replace` atomic so a token can be rotated at most once.
pub trait SessionStore: Send + Sync {
    type Error: Error + Send + Sync + 'static;

    fn insert(&self, session: Session) -> impl Future<Output = Result<(), Self::Error>> + Send;

    fn find(
        &self,
        token_hash: &SessionTokenHash,
    ) -> impl Future<Output = Result<Option<Session>, Self::Error>> + Send;

    /// Returns whether a live session was newly revoked.
    fn revoke(
        &self,
        token_hash: &SessionTokenHash,
        at: SystemTime,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;

    /// Revokes `old` and inserts `new` atomically; returns `false` (changing
    /// nothing) when `old` is unknown or already revoked.
    fn replace(
        &self,
        old: &SessionTokenHash,
        new: Session,
        at: SystemTime,
    ) -> impl Future<Output = Result<bool, Self::Error>> + Send;
}

#[derive(Debug, Default)]
pub struct InMemorySessionStore {
    sessions: Mutex<HashMap<SessionTokenHash, Session>>,
}

impl InMemorySessionStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionTokenHash, Session>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl SessionStore for InMemorySessionStore {
    type Error = Infallible;

    async fn insert(&self, session: Session) -> Result<(), Self::Error> {
        self.lock().insert(session.token_hash.clone(), session);

        Ok(())
    }

    async fn find(&self, token_hash: &SessionTokenHash) -> Result<Option<Session>, Self::Error> {
        Ok(self.lock().get(token_hash).cloned())
    }

    async fn revoke(
        &self,
        token_hash: &SessionTokenHash,
        at: SystemTime,
    ) -> Result<bool, Self::Error> {
        Ok(revoke_live(&mut self.lock(), token_hash, at))
    }

    async fn replace(
        &self,
        old: &SessionTokenHash,
        new: Session,
        at: SystemTime,
    ) -> Result<bool, Self::Error> {
        let mut sessions = self.lock();

        if !revoke_live(&mut sessions, old, at) {
            return Ok(false);
        }

        sessions.insert(new.token_hash.clone(), new);

        Ok(true)
    }
}

fn revoke_live(
    sessions: &mut HashMap<SessionTokenHash, Session>,
    token_hash: &SessionTokenHash,
    at: SystemTime,
) -> bool {
    match sessions.get_mut(token_hash) {
        Some(session) if session.revoked_at.is_none() => {
            session.revoked_at = Some(at);
            true
        }
        _ => false,
    }
}

/// Callers should answer every variant other than `Store` with the same
/// generic "unauthenticated" response; the distinction is for logs and tests.
#[derive(Debug, Error)]
pub enum SessionError<E: Error + 'static> {
    #[error("session token is missing, malformed or unknown")]
    Invalid,
    #[error("session has expired")]
    Expired,
    #[error("session has been revoked")]
    Revoked,
    #[error("session store failure")]
    Store(#[source] E),
}

#[derive(Debug)]
pub struct SessionManager<S, C = SystemClock> {
    store: S,
    clock: C,
    ttl: Duration,
}

impl<S: SessionStore> SessionManager<S> {
    pub fn new(store: S, ttl: Duration) -> Self {
        Self::with_clock(store, SystemClock, ttl)
    }
}

impl<S: SessionStore, C: Clock> SessionManager<S, C> {
    pub fn with_clock(store: S, clock: C, ttl: Duration) -> Self {
        Self { store, clock, ttl }
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    pub async fn create(&self) -> Result<(SessionToken, Session), SessionError<S::Error>> {
        let (token, session) = self.issue(self.clock.now());

        self.store
            .insert(session.clone())
            .await
            .map_err(SessionError::Store)?;

        Ok((token, session))
    }

    /// Accepts the raw cookie value; rejects malformed, unknown, revoked and expired tokens.
    pub async fn validate(&self, token: &str) -> Result<Session, SessionError<S::Error>> {
        let token = SessionToken::parse(token).ok_or(SessionError::Invalid)?;

        self.load_live(&token).await
    }

    /// Issues a fresh token and invalidates the presented one. Call on login and
    /// periodically afterwards to bound the lifetime of any single token.
    pub async fn rotate(
        &self,
        token: &str,
    ) -> Result<(SessionToken, Session), SessionError<S::Error>> {
        let token = SessionToken::parse(token).ok_or(SessionError::Invalid)?;
        let old = self.load_live(&token).await?;
        let now = self.clock.now();
        let (new_token, new_session) = self.issue(now);

        let replaced = self
            .store
            .replace(&old.token_hash, new_session.clone(), now)
            .await
            .map_err(SessionError::Store)?;

        // Another request rotated or revoked the token between our read and write.
        if !replaced {
            return Err(SessionError::Revoked);
        }

        Ok((new_token, new_session))
    }

    /// Idempotent: revoking an unknown or already revoked session succeeds.
    pub async fn revoke(&self, token: &str) -> Result<(), SessionError<S::Error>> {
        let Some(token) = SessionToken::parse(token) else {
            return Ok(());
        };

        self.store
            .revoke(&token.hash(), self.clock.now())
            .await
            .map_err(SessionError::Store)?;

        Ok(())
    }

    async fn load_live(&self, token: &SessionToken) -> Result<Session, SessionError<S::Error>> {
        let session = self
            .store
            .find(&token.hash())
            .await
            .map_err(SessionError::Store)?
            .ok_or(SessionError::Invalid)?;

        if session.revoked_at.is_some() {
            return Err(SessionError::Revoked);
        }

        if self.clock.now() >= session.expires_at {
            return Err(SessionError::Expired);
        }

        Ok(session)
    }

    fn issue(&self, now: SystemTime) -> (SessionToken, Session) {
        let token = SessionToken::generate();
        let session = Session {
            token_hash: token.hash(),
            created_at: now,
            expires_at: now + self.ttl,
            last_rotated_at: now,
            revoked_at: None,
        };

        (token, session)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    const TTL: Duration = Duration::from_secs(3600);

    #[derive(Debug, Clone)]
    struct ManualClock(Arc<Mutex<SystemTime>>);

    impl ManualClock {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(
                SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000),
            )))
        }

        fn advance(&self, by: Duration) {
            *self.0.lock().expect("clock lock") += by;
        }
    }

    impl Clock for ManualClock {
        fn now(&self) -> SystemTime {
            *self.0.lock().expect("clock lock")
        }
    }

    fn manager() -> (
        SessionManager<InMemorySessionStore, ManualClock>,
        ManualClock,
    ) {
        let clock = ManualClock::new();
        let manager =
            SessionManager::with_clock(InMemorySessionStore::default(), clock.clone(), TTL);

        (manager, clock)
    }

    #[test]
    fn tokens_are_random_256_bit_url_safe_values() {
        let first = SessionToken::generate();
        let second = SessionToken::generate();

        assert_ne!(first.expose(), second.expose());
        assert_eq!(first.expose().len(), TOKEN_ENCODED_LEN);
        assert!(SessionToken::parse(first.expose()).is_some());
    }

    #[test]
    fn malformed_tokens_are_not_parsed() {
        for value in [
            "",
            "short",
            &"a".repeat(44),
            &"!".repeat(43),
            &"a".repeat(10_000),
        ] {
            assert!(SessionToken::parse(value).is_none(), "accepted {value:.20}");
        }
    }

    #[test]
    fn secrets_never_appear_in_debug_output() {
        let token = SessionToken::generate();
        let output = format!(
            "{token:?} {:?} {:?}",
            token.hash(),
            Session {
                token_hash: token.hash(),
                created_at: SystemTime::UNIX_EPOCH,
                expires_at: SystemTime::UNIX_EPOCH,
                last_rotated_at: SystemTime::UNIX_EPOCH,
                revoked_at: None,
            }
        );

        assert!(!output.contains(token.expose()));
        assert!(output.contains("REDACTED"));
    }

    #[tokio::test]
    async fn created_session_validates_and_stores_only_the_hash() {
        let (manager, clock) = manager();

        let (token, session) = manager.create().await.expect("create succeeds");
        let validated = manager
            .validate(token.expose())
            .await
            .expect("session is live");

        assert_eq!(validated, session);
        assert_eq!(session.token_hash, token.hash());
        assert_eq!(session.created_at, clock.now());
        assert_eq!(session.last_rotated_at, clock.now());
        assert_eq!(session.expires_at, clock.now() + TTL);
        assert_ne!(session.token_hash.as_bytes(), token.expose().as_bytes());
    }

    #[tokio::test]
    async fn unknown_and_malformed_tokens_are_invalid() {
        let (manager, _) = manager();
        let unknown = SessionToken::generate();

        assert!(matches!(
            manager.validate(unknown.expose()).await,
            Err(SessionError::Invalid)
        ));
        assert!(matches!(
            manager.validate("garbage").await,
            Err(SessionError::Invalid)
        ));
    }

    #[tokio::test]
    async fn expired_session_is_rejected_from_the_expiry_instant() {
        let (manager, clock) = manager();
        let (token, _) = manager.create().await.expect("create succeeds");

        clock.advance(TTL - Duration::from_secs(1));
        assert!(manager.validate(token.expose()).await.is_ok());

        clock.advance(Duration::from_secs(1));
        assert!(matches!(
            manager.validate(token.expose()).await,
            Err(SessionError::Expired)
        ));
        assert!(matches!(
            manager.rotate(token.expose()).await,
            Err(SessionError::Expired)
        ));
    }

    #[tokio::test]
    async fn revoked_session_is_rejected_and_revocation_is_idempotent() {
        let (manager, _) = manager();
        let (token, _) = manager.create().await.expect("create succeeds");

        manager
            .revoke(token.expose())
            .await
            .expect("revoke succeeds");
        manager
            .revoke(token.expose())
            .await
            .expect("revoke is idempotent");
        manager
            .revoke("garbage")
            .await
            .expect("unknown revoke succeeds");

        assert!(matches!(
            manager.validate(token.expose()).await,
            Err(SessionError::Revoked)
        ));
        assert!(matches!(
            manager.rotate(token.expose()).await,
            Err(SessionError::Revoked)
        ));
    }

    #[tokio::test]
    async fn rotation_invalidates_the_replaced_session() {
        let (manager, clock) = manager();
        let (old, _) = manager.create().await.expect("create succeeds");

        clock.advance(Duration::from_secs(600));
        let (new, session) = manager.rotate(old.expose()).await.expect("rotate succeeds");

        assert_ne!(old.expose(), new.expose());
        assert_eq!(session.last_rotated_at, clock.now());
        assert_eq!(session.expires_at, clock.now() + TTL);
        assert!(matches!(
            manager.validate(old.expose()).await,
            Err(SessionError::Revoked)
        ));
        assert!(manager.validate(new.expose()).await.is_ok());
    }

    #[tokio::test]
    async fn a_token_can_only_be_rotated_once() {
        let (manager, _) = manager();
        let (old, _) = manager.create().await.expect("create succeeds");

        manager
            .rotate(old.expose())
            .await
            .expect("first rotation succeeds");

        assert!(matches!(
            manager.rotate(old.expose()).await,
            Err(SessionError::Revoked)
        ));
    }

    #[tokio::test]
    async fn store_replace_refuses_an_already_revoked_session() {
        let (manager, clock) = manager();
        let store = &manager.store;
        let (token, session) = manager.create().await.expect("create succeeds");
        let (_, replacement) = manager.issue(clock.now());

        store
            .revoke(&session.token_hash, clock.now())
            .await
            .expect("revoke succeeds");
        let replaced = store
            .replace(&token.hash(), replacement.clone(), clock.now())
            .await
            .expect("replace succeeds");

        assert!(!replaced);
        assert!(
            store
                .find(&replacement.token_hash)
                .await
                .expect("find succeeds")
                .is_none()
        );
    }

    #[tokio::test]
    async fn full_session_lifecycle() {
        let (manager, clock) = manager();

        let (first, _) = manager.create().await.expect("create succeeds");
        clock.advance(Duration::from_secs(60));
        let (second, _) = manager
            .rotate(first.expose())
            .await
            .expect("rotate succeeds");
        manager
            .revoke(second.expose())
            .await
            .expect("revoke succeeds");

        assert!(matches!(
            manager.validate(first.expose()).await,
            Err(SessionError::Revoked)
        ));
        assert!(matches!(
            manager.validate(second.expose()).await,
            Err(SessionError::Revoked)
        ));
    }
}
