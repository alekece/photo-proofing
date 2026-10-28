use std::{
    collections::HashMap,
    convert::Infallible,
    error::Error,
    fmt,
    future::Future,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, SystemTime},
};

use crate::auth::session::{Clock, SystemClock};

/// Caller-supplied bucket identifier (e.g. client IP plus a token-hash prefix).
/// Redacted in `Debug` because it may embed client addresses or token material.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RateLimitKey(String);

impl RateLimitKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn scoped(&self, scope: &str) -> Self {
        Self(format!("{}|{scope}", self.0))
    }
}

impl fmt::Debug for RateLimitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RateLimitKey([REDACTED])")
    }
}

/// Returned while a key is locked out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locked {
    pub retry_after: Duration,
}

/// Counts attempts per key. Callers reserve an attempt *before* doing any work, so
/// concurrent requests cannot all slip past a check-then-act race, and refund it when
/// the attempt succeeds, leaving only failures counted. Implementations must be
/// atomic; a persistent one is needed to share state across processes.
pub trait RateLimiter: Send + Sync {
    type Error: Error + Send + Sync + 'static;

    /// Atomically refuses when the key is locked out, otherwise counts one attempt
    /// and locks the key once the count reaches the configured maximum.
    fn reserve(
        &self,
        key: &RateLimitKey,
    ) -> impl Future<Output = Result<Result<(), Locked>, Self::Error>> + Send;

    /// Undoes one reservation after a successful attempt. Does not lift a lockout.
    fn refund(&self, key: &RateLimitKey) -> impl Future<Output = Result<(), Self::Error>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub max_failures: u32,
    pub window: Duration,
    pub lockout: Duration,
    /// Upper bound on tracked keys so unauthenticated traffic cannot grow memory.
    pub max_keys: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_failures: 5,
            window: Duration::from_secs(15 * 60),
            lockout: Duration::from_secs(15 * 60),
            max_keys: 10_000,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    window_start: SystemTime,
    failures: u32,
    locked_until: Option<SystemTime>,
}

impl Entry {
    fn is_stale(&self, now: SystemTime, config: &RateLimitConfig) -> bool {
        match self.locked_until {
            Some(until) => until <= now,
            None => self.window_start + config.window <= now,
        }
    }
}

/// Fixed-window limiter with lockout. State is per-process, lost on restart and
/// not shared between instances.
#[derive(Debug)]
pub struct InMemoryRateLimiter<C = SystemClock> {
    config: RateLimitConfig,
    clock: C,
    entries: Mutex<HashMap<RateLimitKey, Entry>>,
}

impl InMemoryRateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self::with_clock(config, SystemClock)
    }
}

impl<C: Clock> InMemoryRateLimiter<C> {
    pub fn with_clock(config: RateLimitConfig, clock: C) -> Self {
        Self {
            config,
            clock,
            entries: Mutex::default(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<RateLimitKey, Entry>> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Makes room for one new key: drops stale entries first, then the entry that
/// matters least (an unlocked one with the oldest window, else the lockout that
/// ends soonest).
fn make_room(
    entries: &mut HashMap<RateLimitKey, Entry>,
    now: SystemTime,
    config: &RateLimitConfig,
) {
    if entries.len() < config.max_keys {
        return;
    }

    entries.retain(|_, entry| !entry.is_stale(now, config));

    if entries.len() < config.max_keys {
        return;
    }

    let victim = entries
        .iter()
        .min_by_key(|(_, entry)| {
            (
                entry.locked_until.is_some(),
                entry.locked_until,
                entry.window_start,
            )
        })
        .map(|(key, _)| key.clone());

    if let Some(key) = victim {
        entries.remove(&key);
    }
}

impl<C: Clock> RateLimiter for InMemoryRateLimiter<C> {
    type Error = Infallible;

    async fn reserve(&self, key: &RateLimitKey) -> Result<Result<(), Locked>, Self::Error> {
        let now = self.clock.now();
        let config = self.config;
        let mut entries = self.lock();

        if entries
            .get(key)
            .is_some_and(|entry| entry.is_stale(now, &config))
        {
            entries.remove(key);
        }

        if !entries.contains_key(key) {
            make_room(&mut entries, now, &config);
        }

        let entry = entries.entry(key.clone()).or_insert(Entry {
            window_start: now,
            failures: 0,
            locked_until: None,
        });

        if let Some(retry_after) = entry
            .locked_until
            .and_then(|until| until.duration_since(now).ok())
        {
            return Ok(Err(Locked { retry_after }));
        }

        entry.failures = entry.failures.saturating_add(1);

        if entry.failures >= config.max_failures {
            entry.locked_until = Some(now + config.lockout);
        }

        Ok(Ok(()))
    }

    async fn refund(&self, key: &RateLimitKey) -> Result<(), Self::Error> {
        if let Some(entry) = self
            .lock()
            .get_mut(key)
            .filter(|entry| entry.locked_until.is_none())
        {
            entry.failures = entry.failures.saturating_sub(1);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sharing::testing::ManualClock;

    const CONFIG: RateLimitConfig = RateLimitConfig {
        max_failures: 3,
        window: Duration::from_secs(60),
        lockout: Duration::from_secs(300),
        max_keys: 4,
    };

    fn limiter() -> (InMemoryRateLimiter<ManualClock>, ManualClock) {
        let clock = ManualClock::new();

        (
            InMemoryRateLimiter::with_clock(CONFIG, clock.clone()),
            clock,
        )
    }

    async fn attempt(
        limiter: &InMemoryRateLimiter<ManualClock>,
        key: &RateLimitKey,
    ) -> Result<(), Locked> {
        limiter.reserve(key).await.expect("infallible")
    }

    #[tokio::test]
    async fn locks_out_once_max_attempts_are_reserved() {
        let (limiter, _) = limiter();
        let key = RateLimitKey::new("ip");

        for _ in 0..CONFIG.max_failures {
            assert!(attempt(&limiter, &key).await.is_ok());
        }

        let locked = attempt(&limiter, &key)
            .await
            .expect_err("further attempts are refused");

        assert_eq!(locked.retry_after, CONFIG.lockout);
    }

    #[tokio::test]
    async fn refunded_attempts_are_not_counted() {
        let (limiter, _) = limiter();
        let key = RateLimitKey::new("ip");

        for _ in 0..10 {
            attempt(&limiter, &key).await.expect("not locked");
            limiter.refund(&key).await.expect("infallible");
        }

        assert!(attempt(&limiter, &key).await.is_ok());
    }

    #[tokio::test]
    async fn refund_does_not_lift_a_lockout() {
        let (limiter, _) = limiter();
        let key = RateLimitKey::new("ip");

        for _ in 0..CONFIG.max_failures {
            let _ = attempt(&limiter, &key).await;
        }
        limiter.refund(&key).await.expect("infallible");

        assert!(attempt(&limiter, &key).await.is_err());
    }

    #[tokio::test]
    async fn keys_are_independent() {
        let (limiter, _) = limiter();
        let key = RateLimitKey::new("a");

        for _ in 0..CONFIG.max_failures {
            let _ = attempt(&limiter, &key).await;
        }

        assert!(attempt(&limiter, &key).await.is_err());
        assert!(attempt(&limiter, &RateLimitKey::new("b")).await.is_ok());
    }

    #[tokio::test]
    async fn recovers_after_lockout() {
        let (limiter, clock) = limiter();
        let key = RateLimitKey::new("ip");

        for _ in 0..CONFIG.max_failures {
            let _ = attempt(&limiter, &key).await;
        }

        clock.advance(CONFIG.lockout - Duration::from_secs(1));
        assert!(attempt(&limiter, &key).await.is_err());

        clock.advance(Duration::from_secs(1));
        assert!(attempt(&limiter, &key).await.is_ok());
    }

    #[tokio::test]
    async fn attempts_outside_the_window_do_not_accumulate() {
        let (limiter, clock) = limiter();
        let key = RateLimitKey::new("ip");

        for _ in 0..CONFIG.max_failures - 1 {
            let _ = attempt(&limiter, &key).await;
        }
        clock.advance(CONFIG.window);

        for _ in 0..CONFIG.max_failures - 1 {
            assert!(attempt(&limiter, &key).await.is_ok());
        }
    }

    #[tokio::test]
    async fn tracked_keys_are_bounded_and_unlocked_noise_is_evicted_first() {
        let (limiter, _) = limiter();
        let locked = RateLimitKey::new("locked");

        for _ in 0..CONFIG.max_failures {
            let _ = attempt(&limiter, &locked).await;
        }

        for index in 0..50 {
            let _ = attempt(&limiter, &RateLimitKey::new(format!("noise-{index}"))).await;
        }

        assert!(limiter.lock().len() <= CONFIG.max_keys);
        assert!(attempt(&limiter, &locked).await.is_err());
    }

    #[test]
    fn keys_are_redacted_in_debug() {
        let key = RateLimitKey::new("203.0.113.9").scoped("abcd");

        assert!(!format!("{key:?}").contains("203.0.113.9"));
    }
}
