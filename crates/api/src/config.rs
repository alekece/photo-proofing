//! Settings of the API process, grouped by concern under [`ApiConfig`].

use std::{
    num::{NonZeroU16, NonZeroU32, NonZeroU64, NonZeroUsize},
    str::FromStr,
    time::Duration,
};

use clap::Args;
use clap_config_fallback::ConfigArgs;
use derive_more::Display;
use pp_core::{
    config::{DEFAULT_MAX_PIXELS, DatabaseConfig, ExportConfig, PhotoConfig},
    time::BoundedDuration,
    url::Origin,
};
use pp_database::jobs::DEFAULT_CLEANUP_BATCH;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::access_code::EncryptionKey;

/// Longest [`CookieDuration`]: far beyond any sensible policy, yet small enough that adding it
/// to the current time cannot overflow a timestamp.
const MAX_COOKIE_MILLIS: u64 = 1000 * 365 * 24 * 60 * 60 * 1000;

/// Longest [`LimiterWindow`]: a day, beyond which a "recent failures" allowance stops meaning
/// anything.
const MAX_LIMITER_WINDOW_MILLIS: u64 = 24 * 60 * 60 * 1000;

/// How long a cookie may live: at least one second, because `Max-Age` has whole-second
/// resolution and anything shorter would round to an instant expiry.
pub type CookieDuration = BoundedDuration<1_000, MAX_COOKIE_MILLIS>;

/// How long failed sign-ins are remembered per peer.
pub type LimiterWindow = BoundedDuration<1_000, MAX_LIMITER_WINDOW_MILLIS>;

/// Minimum time between client comment writes in one gallery.
pub type CommentInterval = BoundedDuration<1_000, 60_000>;

/// Longest [`KeepAlive`]: proxies commonly drop idle connections after a minute or two, so a
/// longer period would only let streams die silently.
const MAX_KEEP_ALIVE_MILLIS: u64 = 5 * 60 * 1000;

/// How often an idle event stream is pinged and its session re-checked.
pub type KeepAlive = BoundedDuration<1_000, MAX_KEEP_ALIVE_MILLIS>;

/// Longest [`UploadRetryAfter`]: a browser told to wait longer would leave the queue idle
/// long after slots free up.
const MAX_UPLOAD_RETRY_AFTER_MILLIS: u64 = 60 * 1000;

/// How long a refused upload is asked to wait; `Retry-After` has whole-second resolution.
pub type UploadRetryAfter = BoundedDuration<1_000, MAX_UPLOAD_RETRY_AFTER_MILLIS>;

/// Longest [`UploadIdleTimeout`]: a client silent for ten minutes is gone, and its slot and
/// staged file are better freed.
const MAX_UPLOAD_IDLE_TIMEOUT_MILLIS: u64 = 10 * 60 * 1000;

/// How long an upload body may stay silent before it is abandoned.
pub type UploadIdleTimeout = BoundedDuration<1_000, MAX_UPLOAD_IDLE_TIMEOUT_MILLIS>;

/// Shortest [`RelayBackoff`]: below this a failing listener would be retried in a hot loop.
const MIN_RELAY_BACKOFF_MILLIS: u64 = 10;

/// Longest [`RelayBackoff`]: the listener is what keeps live streams current, so a longer wait
/// would leave them stale long after the database is back.
const MAX_RELAY_BACKOFF_MILLIS: u64 = 5 * 60 * 1000;

/// A delay before the live event listener reconnects.
pub type RelayBackoff = BoundedDuration<MIN_RELAY_BACKOFF_MILLIS, MAX_RELAY_BACKOFF_MILLIS>;

/// Everything [`Api`](crate::api::Api) needs to start.
#[derive(Args, ConfigArgs, Clone)]
pub struct ApiConfig {
    #[command(flatten)]
    pub http: HttpConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub database: DatabaseConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub photo: PhotoConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub photo_upload: UploadConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub events: EventsConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub session: SessionConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub auth: AuthConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub limiter: LimiterConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub comments: CommentsConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub queue: QueueConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub export: ExportConfig,
    #[command(flatten)]
    #[config(no_flatten)]
    pub gallery_password: GalleryPasswordConfig,
}

/// Optional encryption key for per-gallery 6-digit access codes (`[gallery_password]`).
/// Deliberately omits `Debug` because it holds secret key material.
#[derive(Args, ConfigArgs, Default, Clone)]
pub struct GalleryPasswordConfig {
    /// 256-bit key (64 hex digits or 43 unpadded base64url characters) encrypting gallery access
    /// codes at rest and signing client unlock cookies.
    #[arg(
        long = "gallery-password-key",
        env = "PP_GALLERY_PASSWORD_KEY",
        hide_env_values = true,
        value_parser = EncryptionKey::value_parser()
    )]
    #[config(value_format = EncryptionKey::encode_base64url)]
    pub encryption_key: Option<EncryptionKey>,
}

/// Where the HTTP server listens.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct HttpConfig {
    #[arg(long, env = "PP_HOST", default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, env = "PP_PORT", default_value_t = 8080)]
    pub port: u16,
}

/// The cookie name is empty or contains characters RFC 6265 does not allow in a cookie name.
#[derive(Debug, Error)]
#[error("cookie name must be a non-empty RFC 6265 token")]
pub struct ParseCookieNameError;

/// A valid cookie name, checked once at startup.
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CookieName(String);

impl CookieName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for CookieName {
    type Err = ParseCookieNameError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        // Token characters: visible ASCII except the RFC 2616 separators.
        let is_token = |c: char| c.is_ascii_graphic() && !"()<>@,;:\\\"/[]?={}".contains(c);

        if value.is_empty() || !value.chars().all(is_token) {
            return Err(ParseCookieNameError);
        }

        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for CookieName {
    type Error = ParseCookieNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<CookieName> for String {
    fn from(name: CookieName) -> Self {
        name.0
    }
}

/// Persistent session inactivity policy, shared by API validation and renewal.
#[derive(Args, ConfigArgs, Clone)]
pub struct SessionConfig {
    #[arg(
        long = "session-idle-timeout",
        env = "PP_SESSION_IDLE_TIMEOUT",
        default_value = "30d"
    )]
    pub idle_timeout: CookieDuration,
    #[arg(
        long = "session-cookie-secure",
        env = "PP_SESSION_COOKIE_SECURE",
        action = clap::ArgAction::Set
    )]
    pub secure_cookie: Option<bool>,
    #[arg(
        long = "session-cookie-name",
        env = "PP_SESSION_COOKIE_NAME",
        default_value = "pp_session"
    )]
    pub cookie_name: CookieName,
}

/// The browser-visible origin of the app. The API compares it byte-for-byte with the
/// `Origin` header of state-changing requests, so it must match the URL users actually open.
#[derive(Args, ConfigArgs, Clone)]
pub struct AuthConfig {
    #[arg(
        long = "app-origin",
        env = "PP_APP_ORIGIN",
        default_value = "http://localhost:5173"
    )]
    pub app_origin: Origin,
    /// Password hashes computed at once. Argon2 is memory-hungry, so raise this only with the
    /// memory to match; further sign-in and setup requests are refused with 429.
    #[arg(
        long = "auth-hash-concurrency",
        env = "PP_AUTH_HASH_CONCURRENCY",
        default_value = "2"
    )]
    pub hash_concurrency: NonZeroUsize,
}

/// Allowance of failed sign-ins per peer; see the limiter for how it behaves behind a proxy.
#[derive(Args, ConfigArgs, Clone)]
pub struct LimiterConfig {
    /// Failures a peer may accumulate within one window before sign-in is refused.
    #[arg(
        long = "limiter-max-failures",
        env = "PP_LIMITER_MAX_FAILURES",
        default_value = "10"
    )]
    pub max_failures: NonZeroU32,
    #[arg(
        long = "limiter-window",
        env = "PP_LIMITER_WINDOW",
        default_value = "1m"
    )]
    pub window: LimiterWindow,
    /// Peers tracked at once; bounds memory, and further peers are refused until windows expire.
    #[arg(
        long = "limiter-max-peers",
        env = "PP_LIMITER_MAX_PEERS",
        default_value = "1024"
    )]
    pub max_peers: NonZeroUsize,
}

/// Default shortest pause between two client comment changes in one gallery.
pub const DEFAULT_COMMENT_INTERVAL: Duration = Duration::from_secs(2);

/// Default cap on the comments of one photo.
pub const DEFAULT_COMMENT_LIMIT: NonZeroU16 = NonZeroU16::new(100).unwrap();

/// Client comment abuse controls; photographer replies are exempt.
#[derive(Args, ConfigArgs, Clone)]
pub struct CommentsConfig {
    #[arg(
        long = "comments-min-interval",
        env = "PP_COMMENTS_MIN_INTERVAL",
        default_value = "2s"
    )]
    pub min_interval: CommentInterval,
    #[arg(
        long = "comments-max-per-photo",
        env = "PP_COMMENTS_MAX_PER_PHOTO",
        default_value = "100"
    )]
    pub max_per_photo: NonZeroU16,
}

/// Per-process limits that bound upload memory and metadata parsing.
#[derive(Args, ConfigArgs, Clone)]
pub struct UploadConfig {
    #[arg(
        long = "photo-upload-max-size",
        env = "PP_PHOTO_UPLOAD_MAX_SIZE",
        default_value = "67108864"
    )]
    pub max_size: NonZeroU64,
    #[arg(
        long = "photo-upload-max-pixels",
        env = "PP_PHOTO_UPLOAD_MAX_PIXELS",
        default_value_t = DEFAULT_MAX_PIXELS
    )]
    pub max_pixels: NonZeroU64,
    #[arg(
        long = "photo-upload-concurrency",
        env = "PP_PHOTO_UPLOAD_CONCURRENCY",
        default_value = "4"
    )]
    pub concurrency: NonZeroUsize,
    /// Wait announced to an upload refused because every slot is taken.
    #[arg(
        long = "photo-upload-retry-after",
        env = "PP_PHOTO_UPLOAD_RETRY_AFTER",
        default_value = "1s"
    )]
    pub retry_after: UploadRetryAfter,
    /// How long an upload body may stay silent before the upload is abandoned with a 408.
    #[arg(
        long = "photo-upload-body-idle-timeout",
        env = "PP_PHOTO_UPLOAD_BODY_IDLE_TIMEOUT",
        default_value = "30s"
    )]
    pub body_idle_timeout: UploadIdleTimeout,
}

/// How the jobs the API enqueues are run by the worker.
#[derive(Args, ConfigArgs, Clone)]
pub struct QueueConfig {
    /// Attempts a photo-processing job gets before it fails for good. It is recorded on the job
    /// when the API enqueues it, so only the API needs this setting.
    #[arg(
        long = "queue-max-attempts",
        env = "PP_QUEUE_MAX_ATTEMPTS",
        default_value = "5"
    )]
    pub max_attempts: NonZeroU16,
    /// Most photos one cleanup job names when photos are deleted in bulk.
    #[arg(
        long = "queue-cleanup-batch",
        env = "PP_QUEUE_CLEANUP_BATCH",
        default_value_t = DEFAULT_CLEANUP_BATCH
    )]
    pub cleanup_batch: NonZeroUsize,
}

/// Live event streams: how idle streams are kept open and how the database listener is retried.
#[derive(Args, ConfigArgs, Clone)]
pub struct EventsConfig {
    /// Period of keep-alive comments, also the delay before a stream notices a signed-out session.
    #[arg(
        long = "events-keep-alive",
        env = "PP_EVENTS_KEEP_ALIVE",
        default_value = "15s"
    )]
    pub keep_alive: KeepAlive,
    /// Events buffered per stream before it lags and is told to resync.
    #[arg(
        long = "events-capacity",
        env = "PP_EVENTS_CAPACITY",
        default_value = "1024"
    )]
    pub capacity: NonZeroU16,
    /// First delay before the database listener is reopened after a failure; doubled per failure.
    #[arg(
        long = "events-relay-backoff-start",
        env = "PP_EVENTS_RELAY_BACKOFF_START",
        default_value = "250ms"
    )]
    pub relay_backoff_start: RelayBackoff,
    /// Longest delay between attempts to reopen the database listener; at least the start.
    #[arg(
        long = "events-relay-backoff-cap",
        env = "PP_EVENTS_RELAY_BACKOFF_CAP",
        default_value = "30s"
    )]
    pub relay_backoff_cap: RelayBackoff,
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs, iter,
        path::{Path, PathBuf},
        process::Command,
        time::Duration,
    };

    use clap::Parser;
    use clap_config_fallback::ConfigParser;

    use super::*;

    #[derive(Parser, ConfigParser)]
    struct TestCli {
        #[arg(long, env = "PP_CONFIG")]
        #[config(path)]
        config: Option<PathBuf>,
        #[command(flatten)]
        api: ApiConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_apply_without_config_file_and_photo_upload_limits() {
        let api = parse(&[]).unwrap().api;

        assert_eq!(api.http.host, "127.0.0.1");
        assert_eq!(api.http.port, 8080);
        assert_eq!(
            api.session.idle_timeout.get(),
            Duration::from_secs(30 * 86400)
        );
        assert!(!api.session.secure_cookie.unwrap_or_default());
        assert_eq!(api.auth.app_origin.as_str(), "http://localhost:5173");
        assert_eq!(api.limiter.max_failures.get(), 10);
        assert_eq!(api.limiter.window.get(), Duration::from_secs(60));
        assert_eq!(api.limiter.max_peers.get(), 1024);
        assert_eq!(api.comments.min_interval.get(), Duration::from_secs(2));
        assert_eq!(api.comments.max_per_photo.get(), 100);
        assert_eq!(api.photo_upload.max_size.get(), 64 * 1024 * 1024);
        assert_eq!(api.photo_upload.max_pixels.get(), 100_000_000);
        assert_eq!(api.photo_upload.concurrency.get(), 4);
        assert_eq!(api.photo_upload.retry_after.get(), Duration::from_secs(1));
        assert_eq!(api.photo.dir.as_path(), Path::new("./data/photos"));
        assert_eq!(api.events.keep_alive.get(), Duration::from_secs(15));
        assert_eq!(api.events.capacity.get(), 1024);
        assert_eq!(
            api.events.relay_backoff_start.get(),
            Duration::from_millis(250)
        );
        assert_eq!(api.events.relay_backoff_cap.get(), Duration::from_secs(30));
        assert_eq!(
            api.photo_upload.body_idle_timeout.get(),
            Duration::from_secs(30)
        );
        assert_eq!(api.auth.hash_concurrency.get(), 2);
        assert_eq!(api.queue.max_attempts.get(), 5);
    }

    #[test]
    fn tunable_limits_are_read_from_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(
            &path,
            "[photo_upload]\nbody_idle_timeout = '1m'\n[auth]\nhash_concurrency = 4\n[queue]\nmax_attempts = 9\n\
             [events]\ncapacity = 64\nrelay_backoff_start = '1s'\nrelay_backoff_cap = '2m'\n",
        )
        .unwrap();
        let api = parse(&["--config", path.to_str().unwrap()]).unwrap().api;

        assert_eq!(
            api.photo_upload.body_idle_timeout.get(),
            Duration::from_secs(60)
        );
        assert_eq!(api.auth.hash_concurrency.get(), 4);
        assert_eq!(api.queue.max_attempts.get(), 9);
        assert_eq!(api.events.capacity.get(), 64);
        assert_eq!(api.events.relay_backoff_start.get(), Duration::from_secs(1));
        assert_eq!(api.events.relay_backoff_cap.get(), Duration::from_secs(120));
    }

    #[test]
    fn the_queue_section_may_be_shared_with_the_worker() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "[queue]\nmax_attempts = 7\npoll_interval = '5s'\n").unwrap();

        let api = parse(&["--config", path.to_str().unwrap()]).unwrap().api;

        assert_eq!(api.queue.max_attempts.get(), 7);
    }

    #[test]
    fn tunable_limits_reject_out_of_range_values() {
        for args in [
            ["--photo-upload-body-idle-timeout", "0s"],
            ["--photo-upload-body-idle-timeout", "1h"],
            ["--auth-hash-concurrency", "0"],
            ["--queue-max-attempts", "0"],
            ["--events-capacity", "0"],
            ["--events-capacity", "70000"],
            ["--events-relay-backoff-start", "1ms"],
            ["--events-relay-backoff-cap", "1h"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn events_settings_are_read_from_config_and_validated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "[events]\nkeep_alive = '30s'\ncapacity = 10\n").unwrap();
        let events = parse(&["--config", path.to_str().unwrap()])
            .unwrap()
            .api
            .events;

        assert_eq!(events.keep_alive.get(), Duration::from_secs(30));
        assert_eq!(events.capacity.get(), 10);

        for args in [
            ["--events-keep-alive", "0s"],
            ["--events-keep-alive", "10m"],
            ["--events-capacity", "0"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn invalid_config_file_values_are_rejected_not_replaced_by_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");

        for value in ["capacity = 0", "keep_alive = '0s'"] {
            fs::write(&path, format!("[events]\n{value}\n")).unwrap();

            assert!(
                parse(&["--config", path.to_str().unwrap()]).is_err(),
                "{value}"
            );
        }
    }

    #[test]
    fn photo_upload_limits_are_read_from_config_and_cli() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(
            &path,
            "[photo_upload]\nmax_size = 1234\nmax_pixels = 5678\nconcurrency = 2\nretry_after = '5s'\n",
        )
        .unwrap();
        let args = ["--config", path.to_str().unwrap()];
        let config = parse(&args).unwrap().api.photo_upload;

        assert_eq!(config.max_size.get(), 1234);
        assert_eq!(config.max_pixels.get(), 5678);
        assert_eq!(config.concurrency.get(), 2);
        assert_eq!(config.retry_after.get(), Duration::from_secs(5));

        let config = parse(&[args[0], args[1], "--photo-upload-max-size", "4321"])
            .unwrap()
            .api
            .photo_upload;

        assert_eq!(config.max_size.get(), 4321);
        assert_eq!(config.max_pixels.get(), 5678);
        assert_eq!(config.concurrency.get(), 2);
    }

    #[test]
    fn photo_upload_concurrency_and_retry_after_reject_out_of_range_values() {
        for args in [
            ["--photo-upload-concurrency", "0"],
            ["--photo-upload-retry-after", "0s"],
            ["--photo-upload-retry-after", "2m"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn config_file_overrides_default_and_cli_overrides_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "port = 9000\n").unwrap();
        let config = ["--config", path.to_str().unwrap()];

        assert_eq!(parse(&config).unwrap().api.http.port, 9000);
        assert_eq!(
            parse(&[config[0], config[1], "--port", "9200"])
                .unwrap()
                .api
                .http
                .port,
            9200
        );
    }

    #[test]
    fn invalid_port_is_rejected() {
        assert!(parse(&["--port", "abc"]).is_err());
        assert!(parse(&["--port", "70000"]).is_err());
    }

    #[test]
    fn sectioned_human_durations_and_cli_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(
            &path,
            "[session]\nidle_timeout = '1h 30m'\n[limiter]\nmax_failures = 3\nwindow = '5m'\n",
        )
        .unwrap();
        let args = ["--config", path.to_str().unwrap()];

        let api = parse(&args).unwrap().api;

        assert_eq!(api.session.idle_timeout.get(), Duration::from_secs(5400));
        assert_eq!(api.limiter.max_failures.get(), 3);
        assert_eq!(api.limiter.window.get(), Duration::from_secs(300));

        let api = parse(&[args[0], args[1], "--session-idle-timeout", "2h"])
            .unwrap()
            .api;

        assert_eq!(api.session.idle_timeout.get(), Duration::from_secs(7200));
    }

    #[test]
    fn secure_cookie_is_read_from_config_and_cli() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "[session]\nsecure_cookie = true\n").unwrap();
        let args = ["--config", path.to_str().unwrap()];

        assert!(
            parse(&args)
                .unwrap()
                .api
                .session
                .secure_cookie
                .unwrap_or_default()
        );
        assert!(
            !parse(&[args[0], args[1], "--session-cookie-secure", "false"])
                .unwrap()
                .api
                .session
                .secure_cookie
                .unwrap_or_default()
        );
    }

    #[test]
    fn invalid_session_timeouts_and_origins_are_rejected_at_parse_time() {
        for value in ["-1s", "nonsense", "0s", "500ms", "999999999999999s"] {
            assert!(
                parse(&["--session-idle-timeout", value]).is_err(),
                "{value}"
            );
        }

        for origin in [
            "null",
            "ftp://localhost",
            "http://localhost/path",
            "http://user:pass@localhost",
        ] {
            assert!(parse(&["--app-origin", origin]).is_err(), "{origin}");
        }
    }

    #[test]
    fn invalid_limiter_settings_are_rejected_at_parse_time() {
        for args in [
            ["--limiter-max-failures", "0"],
            ["--comments-max-per-photo", "0"],
            ["--comments-min-interval", "0s"],
            ["--comments-min-interval", "61s"],
            ["--limiter-max-peers", "0"],
            ["--limiter-window", "0s"],
            ["--limiter-window", "2d"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn environment_precedence_is_process_isolated() {
        if env::var_os("PP_TEST_CONFIG_CHILD").is_some() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("c.toml");
            fs::write(
                &path,
                "[session]\nidle_timeout = '1h'\nsecure_cookie = false\n",
            )
            .unwrap();
            let args = ["--config", path.to_str().unwrap()];

            let api = parse(&args).unwrap().api;

            assert_eq!(api.session.idle_timeout.get(), Duration::from_secs(7200));
            assert!(api.session.secure_cookie.unwrap_or_default());

            let api = parse(&[
                args[0],
                args[1],
                "--session-idle-timeout",
                "3h",
                "--session-cookie-secure",
                "false",
            ])
            .unwrap()
            .api;

            assert_eq!(api.session.idle_timeout.get(), Duration::from_secs(10800));
            assert!(!api.session.secure_cookie.unwrap_or_default());

            return;
        }

        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "config::tests::environment_precedence_is_process_isolated",
            ])
            .env("PP_TEST_CONFIG_CHILD", "1")
            .env("PP_SESSION_IDLE_TIMEOUT", "2h")
            .env("PP_SESSION_COOKIE_SECURE", "true")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    #[test]
    fn cookie_names_must_be_rfc_6265_tokens() {
        assert!("pp_session".parse::<CookieName>().is_ok());
        assert!("".parse::<CookieName>().is_err());
        assert!("a b".parse::<CookieName>().is_err());
        assert!("a=b".parse::<CookieName>().is_err());
        assert!("a;b".parse::<CookieName>().is_err());
    }

    #[test]
    fn gallery_password_key_is_optional_and_validated_when_present() {
        if env::var_os("PP_TEST_HELP_SECRET_CHILD").is_some() {
            let help = parse(&["--help"]).err().unwrap().to_string();
            assert!(!help.contains(&"5a".repeat(32)));

            let bad_env = Command::new(env::current_exe().unwrap())
                .args([
                    "--exact",
                    "config::tests::gallery_password_key_is_optional_and_validated_when_present",
                ])
                .env_remove("PP_TEST_HELP_SECRET_CHILD")
                .env("PP_TEST_BAD_ENV_CHILD", "1")
                .env("PP_GALLERY_PASSWORD_KEY", "secret-env-bad")
                .output()
                .unwrap();
            assert!(bad_env.status.success());
            return;
        }

        if env::var_os("PP_TEST_BAD_ENV_CHILD").is_some() {
            let env_err = parse(&[]).err().unwrap().to_string();
            assert!(!env_err.contains("secret-env-bad"), "{env_err}");
            return;
        }

        let api = parse(&[]).unwrap().api;
        assert!(api.gallery_password.encryption_key.is_none());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        let hex_key = "0123456789abcdef".repeat(4);

        fs::write(
            &path,
            format!("[gallery_password]\nencryption_key = '{hex_key}'\n"),
        )
        .unwrap();

        let api = parse(&["--config", path.to_str().unwrap()]).unwrap().api;
        assert!(api.gallery_password.encryption_key.is_some());

        for bad in ["secret-too-short", &"zz".repeat(32)] {
            let cli_err = parse(&["--gallery-password-key", bad]).err().unwrap();
            assert!(!cli_err.to_string().contains(bad), "{bad}");

            fs::write(
                &path,
                format!("[gallery_password]\nencryption_key = '{bad}'\n"),
            )
            .unwrap();
            let toml_err = parse(&["--config", path.to_str().unwrap()]).err().unwrap();
            assert!(!toml_err.to_string().contains(bad), "{bad}");
        }

        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "config::tests::gallery_password_key_is_optional_and_validated_when_present",
            ])
            .env("PP_TEST_HELP_SECRET_CHILD", "1")
            .env("PP_GALLERY_PASSWORD_KEY", "5a".repeat(32))
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
