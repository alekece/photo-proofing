use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, MutexGuard, PoisonError},
    time::{Duration, Instant},
};

/// Fixed-window limiter of failed sign-ins per peer: 10 failures per minute.
///
/// State is in-process, bounded to 1024 peers, and resets on restart. Behind a proxy every
/// client shares the proxy's address, so they share one allowance.
#[derive(Default)]
pub struct AttemptLimiter {
    windows: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl AttemptLimiter {
    const WINDOW: Duration = Duration::from_secs(60);
    const MAX_FAILURES: u32 = 10;
    const MAX_PEERS: usize = 1024;

    pub fn allows(&self, peer: IpAddr) -> bool {
        let mut windows = self.lock();
        let now = Instant::now();

        windows.retain(|_, (start, _)| now.duration_since(*start) < Self::WINDOW);

        let failures = windows.get(&peer).map_or(0, |(_, count)| *count);

        failures < Self::MAX_FAILURES
            && (windows.len() < Self::MAX_PEERS || windows.contains_key(&peer))
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
    use std::net::Ipv4Addr;

    use super::*;

    fn peer(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, last))
    }

    #[test]
    fn a_peer_is_blocked_after_ten_failures_without_affecting_others() {
        let limiter = AttemptLimiter::default();

        for _ in 0..AttemptLimiter::MAX_FAILURES {
            assert!(limiter.allows(peer(1)));

            limiter.record(peer(1));
        }

        assert!(!limiter.allows(peer(1)));
        assert!(limiter.allows(peer(2)));
    }

    #[test]
    fn new_peers_are_refused_once_the_table_is_full() {
        let limiter = AttemptLimiter::default();

        for index in 0..AttemptLimiter::MAX_PEERS {
            limiter.record(IpAddr::V4(Ipv4Addr::from(u32::try_from(index).unwrap())));
        }

        assert!(!limiter.allows(peer(1)));
        assert!(limiter.allows(IpAddr::V4(Ipv4Addr::from(0))));
    }
}
