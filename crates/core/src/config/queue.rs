use std::time::Duration;

use clap::Args;
use clap_config_fallback::ConfigArgs;
use thiserror::Error;

use crate::queue::{QueuePolicy, backoff::Backoff};

/// Bounds durations so that adding them to a timestamp can never overflow.
const MAX_DURATION_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct QueueConfig {
    #[arg(long, env = "EG_QUEUE_BASE_BACKOFF_SECS", default_value_t = 5)]
    pub queue_base_backoff_secs: u64,
    #[arg(long, env = "EG_QUEUE_BACKOFF_FACTOR", default_value_t = 2)]
    pub queue_backoff_factor: u32,
    #[arg(long, env = "EG_QUEUE_MAX_BACKOFF_SECS", default_value_t = 300)]
    pub queue_max_backoff_secs: u64,
    #[arg(long, env = "EG_QUEUE_MAX_ATTEMPTS", default_value_t = 5)]
    pub queue_max_attempts: u32,
    #[arg(long, env = "EG_QUEUE_LEASE_SECS", default_value_t = 60)]
    pub queue_lease_secs: u64,
    #[arg(long, env = "EG_QUEUE_HEARTBEAT_INTERVAL_SECS", default_value_t = 20)]
    pub queue_heartbeat_interval_secs: u64,
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum QueueConfigError {
    #[error("queue lease must be greater than zero")]
    ZeroLease,
    #[error("queue heartbeat interval must be greater than zero and shorter than the lease")]
    HeartbeatNotShorterThanLease,
    #[error("queue max attempts must be at least 1")]
    ZeroAttempts,
    #[error("queue backoff factor must be at least 1")]
    ZeroBackoffFactor,
    #[error("queue lease and max backoff must not exceed {MAX_DURATION_SECS} seconds")]
    DurationTooLong,
    #[error("queue max backoff must not be shorter than the base backoff")]
    CapBelowBase,
}

impl QueueConfig {
    pub fn policy(&self) -> Result<QueuePolicy, QueueConfigError> {
        if self.queue_lease_secs == 0 {
            return Err(QueueConfigError::ZeroLease);
        }
        if self.queue_lease_secs > MAX_DURATION_SECS
            || self.queue_max_backoff_secs > MAX_DURATION_SECS
        {
            return Err(QueueConfigError::DurationTooLong);
        }
        if self.queue_heartbeat_interval_secs == 0
            || self.queue_heartbeat_interval_secs >= self.queue_lease_secs
        {
            return Err(QueueConfigError::HeartbeatNotShorterThanLease);
        }
        if self.queue_max_attempts == 0 {
            return Err(QueueConfigError::ZeroAttempts);
        }
        if self.queue_backoff_factor == 0 {
            return Err(QueueConfigError::ZeroBackoffFactor);
        }
        if self.queue_max_backoff_secs < self.queue_base_backoff_secs {
            return Err(QueueConfigError::CapBelowBase);
        }

        Ok(QueuePolicy {
            backoff: Backoff {
                base: Duration::from_secs(self.queue_base_backoff_secs),
                factor: self.queue_backoff_factor,
                max: Duration::from_secs(self.queue_max_backoff_secs),
            },
            max_attempts: self.queue_max_attempts,
            lease: Duration::from_secs(self.queue_lease_secs),
            heartbeat_interval: Duration::from_secs(self.queue_heartbeat_interval_secs),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, iter, path::PathBuf};

    use clap::Parser;
    use clap_config_fallback::ConfigParser;

    use super::*;

    #[derive(Parser, ConfigParser)]
    struct TestCli {
        #[arg(long, env = "EG_CONFIG")]
        #[config(path)]
        config: Option<PathBuf>,
        #[command(flatten)]
        queue: QueueConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_match_the_documented_policy() {
        let policy = parse(&[]).expect("parse").queue.policy().expect("valid");

        assert_eq!(policy.backoff.base, Duration::from_secs(5));
        assert_eq!(policy.backoff.factor, 2);
        assert_eq!(policy.backoff.max, Duration::from_secs(300));
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.lease, Duration::from_secs(60));
        assert_eq!(policy.heartbeat_interval, Duration::from_secs(20));
    }

    #[test]
    fn config_file_overrides_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("c.toml");
        fs::write(&path, "queue_max_attempts = 9\n").expect("write");

        let cli = parse(&["--config", path.to_str().expect("utf8")]).expect("parse");

        assert_eq!(cli.queue.queue_max_attempts, 9);
    }

    #[test]
    fn heartbeat_must_be_shorter_than_lease() {
        let cli = parse(&["--queue-heartbeat-interval-secs", "60"]).expect("parse");

        assert_eq!(
            cli.queue.policy(),
            Err(QueueConfigError::HeartbeatNotShorterThanLease)
        );
    }

    #[test]
    fn degenerate_values_are_rejected() {
        let cases = [
            (
                ["--queue-max-attempts", "0"],
                QueueConfigError::ZeroAttempts,
            ),
            (
                ["--queue-backoff-factor", "0"],
                QueueConfigError::ZeroBackoffFactor,
            ),
            (
                ["--queue-max-backoff-secs", "1"],
                QueueConfigError::CapBelowBase,
            ),
        ];

        for (args, expected) in cases {
            let cli = parse(&args).expect("parse");

            assert_eq!(cli.queue.policy(), Err(expected));
        }
    }
}
