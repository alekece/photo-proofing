//! Settings of the worker process, grouped by concern under [`WorkerConfig`].

use std::num::{NonZeroU8, NonZeroU64, NonZeroUsize};

use clap::Args;
use clap_config_fallback::ConfigArgs;
use pp_core::{
    config::{DEFAULT_MAX_PIXELS, DatabaseConfig},
    time::BoundedDuration,
};
use pp_database::jobs::Backoff;

use crate::runtime::RuntimeOptions;

/// Longest configurable queue duration: a day, beyond which a lease or delay stops meaning
/// anything for jobs that are meant to be short.
const MAX_QUEUE_MILLIS: u64 = 24 * 60 * 60 * 1000;

/// A queue duration of at least one second, the resolution operators think in.
pub type QueueDuration = BoundedDuration<1_000, MAX_QUEUE_MILLIS>;

/// Everything the [`Worker`](crate::runtime::Worker) needs to start.
#[derive(Args, ConfigArgs, Clone)]
pub struct WorkerConfig {
    #[command(flatten)]
    #[config(no_flatten)]
    pub database: DatabaseConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub queue: QueueConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub process: ProcessConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub cleanup: CleanupConfig,
}

/// How the worker runs its maintenance: orphan sweeps, export expiry and archive sweeps.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct CleanupConfig {
    /// A file younger than this is never swept, so an upload still being finished is safe.
    #[arg(
        long = "cleanup-orphan-grace",
        env = "PP_CLEANUP_ORPHAN_GRACE",
        default_value = "24h"
    )]
    pub orphan_grace: QueueDuration,
    /// Time between maintenance runs. The first run starts with the worker; a run that is late,
    /// because it overran or waited for a free slot, skips the ticks it missed. Run one worker
    /// to keep maintenance from running twice at once.
    #[arg(
        long = "cleanup-sweep-interval",
        env = "PP_CLEANUP_SWEEP_INTERVAL",
        default_value = "5m"
    )]
    pub sweep_interval: QueueDuration,
}

/// Limits on the originals the worker renders into proof derivatives.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct ProcessConfig {
    /// Larger originals fail without being decoded; keep it in line with the API's upload limit.
    #[arg(
        long = "process-max-pixels",
        env = "PP_PROCESS_MAX_PIXELS",
        default_value_t = DEFAULT_MAX_PIXELS
    )]
    pub max_pixels: NonZeroU64,
}

/// How the worker polls, leases and retries jobs.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct QueueConfig {
    /// Wait between polls once the queue is empty.
    #[arg(
        long = "queue-poll-interval",
        env = "PP_QUEUE_POLL_INTERVAL",
        default_value = "5s"
    )]
    pub poll_interval: QueueDuration,
    /// How long a claim lasts without renewal. It is renewed three times per lease, so it must
    /// cover a few missed renewals, not the slowest job.
    #[arg(long = "queue-lease", env = "PP_QUEUE_LEASE", default_value = "60s")]
    pub lease: QueueDuration,
    /// Jobs running at once; also bounds the worker's memory and database connections.
    #[arg(
        long = "queue-concurrency",
        env = "PP_QUEUE_CONCURRENCY",
        default_value = "2"
    )]
    pub concurrency: NonZeroU8,
    /// Delay after the first failure; it doubles with each further one.
    #[arg(
        long = "queue-backoff-base",
        env = "PP_QUEUE_BACKOFF_BASE",
        default_value = "10s"
    )]
    pub backoff_base: QueueDuration,
    /// Longest retry delay.
    #[arg(
        long = "queue-backoff-cap",
        env = "PP_QUEUE_BACKOFF_CAP",
        default_value = "5m"
    )]
    pub backoff_cap: QueueDuration,
    /// How long running jobs may keep going after a stop signal; whatever is left is recovered
    /// once its lease expires.
    #[arg(
        long = "queue-shutdown-grace",
        env = "PP_QUEUE_SHUTDOWN_GRACE",
        default_value = "30s"
    )]
    pub shutdown_grace: QueueDuration,
}

impl From<QueueConfig> for RuntimeOptions {
    fn from(config: QueueConfig) -> Self {
        Self {
            poll_interval: config.poll_interval.get(),
            lease: config.lease.get(),
            concurrency: NonZeroUsize::from(config.concurrency),
            shutdown_grace: config.shutdown_grace.get(),
            backoff: Backoff::new(config.backoff_base.get(), config.backoff_cap.get()),
        }
    }
}

impl QueueConfig {
    /// Connections for `concurrency` running jobs plus one for the poll loop.
    pub fn pool_size(&self) -> u32 {
        u32::from(self.concurrency.get()) + 1
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, iter, path::PathBuf, time::Duration};

    use clap::Parser;
    use clap_config_fallback::ConfigParser;

    use super::*;

    #[derive(Parser, ConfigParser)]
    struct TestCli {
        #[arg(long, env = "PP_CONFIG")]
        #[config(path)]
        config: Option<PathBuf>,
        #[command(flatten)]
        worker: WorkerConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_apply_without_config_file() {
        let queue = parse(&[]).unwrap().worker.queue;

        assert_eq!(queue.poll_interval.get(), Duration::from_secs(5));
        assert_eq!(queue.lease.get(), Duration::from_secs(60));
        assert_eq!(queue.concurrency.get(), 2);
        assert_eq!(queue.backoff_cap.get(), Duration::from_secs(300));
    }

    #[test]
    fn queue_section_is_read_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "[queue]\nlease = \"2m\"\nconcurrency = 4\n").unwrap();

        let queue = parse(&["--config", path.to_str().unwrap()])
            .unwrap()
            .worker
            .queue;

        assert_eq!(queue.lease.get(), Duration::from_secs(120));
        assert_eq!(queue.concurrency.get(), 4);
    }

    #[test]
    fn process_limit_defaults_to_the_upload_limit_and_reads_config_file() {
        assert_eq!(
            parse(&[]).unwrap().worker.process.max_pixels,
            DEFAULT_MAX_PIXELS
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "[process]\nmax_pixels = 5678\n").unwrap();

        let config = parse(&["--config", path.to_str().unwrap()])
            .unwrap()
            .worker
            .process;

        assert_eq!(config.max_pixels.get(), 5678);
        assert!(parse(&["--process-max-pixels", "0"]).is_err());
    }

    #[test]
    fn unusable_queue_settings_are_rejected_at_parse_time() {
        for args in [
            ["--queue-concurrency", "0"],
            ["--queue-lease", "0s"],
            ["--queue-poll-interval", "2d"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
}
