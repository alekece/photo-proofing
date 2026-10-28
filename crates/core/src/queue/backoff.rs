use std::time::Duration;

/// Exponential backoff: `base * factor^(attempt - 1)`, capped at `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    pub base: Duration,
    pub factor: u32,
    pub max: Duration,
}

/// Perturbs a computed delay. The core stays deterministic by defaulting to
/// [`NoJitter`]; production may inject a randomised source.
pub trait Jitter: Send + Sync {
    fn apply(&self, delay: Duration) -> Duration;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoJitter;

impl Jitter for NoJitter {
    fn apply(&self, delay: Duration) -> Duration {
        delay
    }
}

impl Backoff {
    /// `attempt` is the 1-based number of the attempt that just failed.
    pub fn delay(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1);
        let multiplier = self.factor.checked_pow(exponent);

        multiplier
            .and_then(|multiplier| self.base.checked_mul(multiplier))
            .map_or(self.max, |delay| delay.min(self.max))
    }

    /// The jittered delay never exceeds the cap, so `max` stays a hard limit.
    pub fn delay_with_jitter(&self, attempt: u32, jitter: &dyn Jitter) -> Duration {
        jitter.apply(self.delay(attempt)).min(self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BACKOFF: Backoff = Backoff {
        base: Duration::from_secs(5),
        factor: 2,
        max: Duration::from_secs(300),
    };

    struct Doubling;

    impl Jitter for Doubling {
        fn apply(&self, delay: Duration) -> Duration {
            delay * 2
        }
    }

    #[test]
    fn schedule_doubles_until_the_cap() {
        let secs: Vec<u64> = (1..=8).map(|n| BACKOFF.delay(n).as_secs()).collect();

        assert_eq!(secs, [5, 10, 20, 40, 80, 160, 300, 300]);
    }

    #[test]
    fn huge_attempt_numbers_saturate_at_the_cap() {
        assert_eq!(BACKOFF.delay(u32::MAX), BACKOFF.max);
    }

    #[test]
    fn jitter_cannot_exceed_the_cap() {
        let delay = BACKOFF.delay_with_jitter(7, &Doubling);

        assert_eq!(delay, BACKOFF.max);
    }

    #[test]
    fn no_jitter_is_identity() {
        assert_eq!(
            BACKOFF.delay_with_jitter(2, &NoJitter),
            Duration::from_secs(10)
        );
    }
}
