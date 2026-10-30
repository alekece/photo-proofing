use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, MutexGuard, PoisonError},
    time::Instant,
};

use crate::config::LimiterConfig;

/// Fixed-window limiter of failed sign-ins per peer, as set by [`LimiterConfig`].
///
/// State is in-process, bounded to the configured number of peers, and resets on restart.
/// Behind a proxy every client shares the proxy's address, so they share one allowance.
pub struct AttemptLimiter {
    config: LimiterConfig,
    windows: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl AttemptLimiter {
    pub fn new(config: LimiterConfig) -> Self {
        Self {
            config,
            windows: Mutex::default(),
        }
    }

    pub fn allows(&self, peer: IpAddr) -> bool {
        let mut windows = self.lock();
        let now = Instant::now();
        let window = self.config.window.get();

        windows.retain(|_, (start, _)| now.duration_since(*start) < window);

        let failures = windows.get(&peer).map_or(0, |(_, count)| *count);

        failures < self.config.max_failures.get()
            && (windows.len() < self.config.max_peers.get() || windows.contains_key(&peer))
    }

    pub fn record(&self, peer: IpAddr) {
        self.lock().entry(peer).or_insert((Instant::now(), 0)).1 += 1;
    }

    // The map holds plain counters, so state left by a panicking holder is still valid.
    fn lock(&self) -> MutexGuard<'_, HashMap<IpAddr, (Instant, u32)>> {
        self.windows.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::Ipv4Addr,
        num::{NonZeroU32, NonZeroUsize},
    };

    use super::*;

    fn limiter() -> AttemptLimiter {
        AttemptLimiter::new(LimiterConfig {
            max_failures: NonZeroU32::new(3).unwrap(),
            window: "1m".parse().unwrap(),
            max_peers: NonZeroUsize::new(4).unwrap(),
        })
    }

    fn peer(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
    }

    #[test]
    fn a_peer_is_blocked_after_the_configured_failures_without_affecting_others() {
        let limiter = limiter();

        for _ in 0..3 {
            assert!(limiter.allows(peer(1)));

            limiter.record(peer(1));
        }

        assert!(!limiter.allows(peer(1)));
        assert!(limiter.allows(peer(2)));
    }

    #[test]
    fn new_peers_are_refused_once_the_table_is_full() {
        let limiter = limiter();

        for last in 1..=4 {
            limiter.record(peer(last));
        }

        assert!(!limiter.allows(peer(5)));
        assert!(limiter.allows(peer(1)));
    }
}
