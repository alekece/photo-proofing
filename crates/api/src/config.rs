//! Settings of the API process, grouped by concern under [`ApiConfig`].

use std::{
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    str::FromStr,
};

use clap::Args;
use clap_config_fallback::ConfigArgs;
use derive_more::Display;
use pp_core::{
    config::{DEFAULT_MAX_PIXELS, DatabaseConfig, PhotoConfig},
    time::BoundedDuration,
    url::Origin,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Longest [`CookieDuration`]: far beyond any sensible policy, yet small enough that adding it
/// to the current time cannot overflow a timestamp.
const MAX_COOKIE_SECS: u64 = 1000 * 365 * 24 * 60 * 60;

/// Longest [`LimiterWindow`]: a day, beyond which a "recent failures" allowance stops meaning
/// anything.
const MAX_LIMITER_WINDOW_SECS: u64 = 24 * 60 * 60;

/// How long a cookie may live: at least one second, because `Max-Age` has whole-second
/// resolution and anything shorter would round to an instant expiry.
pub type CookieDuration = BoundedDuration<1, MAX_COOKIE_SECS>;

/// How long failed sign-ins are remembered per peer.
pub type LimiterWindow = BoundedDuration<1, MAX_LIMITER_WINDOW_SECS>;

/// Longest [`KeepAlive`]: proxies commonly drop idle connections after a minute or two, so a
/// longer period would only let streams die silently.
const MAX_KEEP_ALIVE_SECS: u64 = 5 * 60;

/// How often an idle event stream is pinged and its session re-checked.
pub type KeepAlive = BoundedDuration<1, MAX_KEEP_ALIVE_SECS>;

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
}

/// Live event streams: how idle streams are kept open and how many may be open at once.
#[derive(Args, ConfigArgs, Clone)]
pub struct EventsConfig {
    /// Period of keep-alive comments, also the delay before a stream notices a signed-out session.
    #[arg(
        long = "events-keep-alive",
        env = "PP_EVENTS_KEEP_ALIVE",
        default_value = "15s"
    )]
    pub keep_alive: KeepAlive,
    /// Streams open at once across all workspaces; each holds an HTTP connection.
    #[arg(
        long = "events-max-streams",
        env = "PP_EVENTS_MAX_STREAMS",
        default_value = "256"
    )]
    pub max_streams: NonZeroUsize,
    /// Streams one workspace may hold, so one workspace cannot take every slot.
    #[arg(
        long = "events-max-streams-per-workspace",
        env = "PP_EVENTS_MAX_STREAMS_PER_WORKSPACE",
        default_value = "32"
    )]
    pub max_streams_per_workspace: NonZeroUsize,
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
        assert_eq!(api.photo_upload.max_size.get(), 64 * 1024 * 1024);
        assert_eq!(api.photo_upload.max_pixels.get(), 100_000_000);
        assert_eq!(api.photo_upload.concurrency.get(), 4);
        assert_eq!(api.photo.dir.as_path(), Path::new("./data/photos"));
        assert_eq!(api.events.keep_alive.get(), Duration::from_secs(15));
        assert_eq!(api.events.max_streams.get(), 256);
        assert_eq!(api.events.max_streams_per_workspace.get(), 32);
    }

    #[test]
    fn events_settings_are_read_from_config_and_validated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(
            &path,
            "[events]\nkeep_alive = '30s'\nmax_streams = 10\nmax_streams_per_workspace = 2\n",
        )
        .unwrap();
        let events = parse(&["--config", path.to_str().unwrap()])
            .unwrap()
            .api
            .events;

        assert_eq!(events.keep_alive.get(), Duration::from_secs(30));
        assert_eq!(events.max_streams.get(), 10);
        assert_eq!(events.max_streams_per_workspace.get(), 2);

        for args in [
            ["--events-keep-alive", "0s"],
            ["--events-keep-alive", "10m"],
            ["--events-max-streams", "0"],
            ["--events-max-streams-per-workspace", "0"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn photo_upload_limits_are_read_from_config_and_cli() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(
            &path,
            "[photo_upload]\nmax_size = 1234\nmax_pixels = 5678\nconcurrency = 2\n",
        )
        .unwrap();
        let args = ["--config", path.to_str().unwrap()];
        let config = parse(&args).unwrap().api.photo_upload;

        assert_eq!(config.max_size.get(), 1234);
        assert_eq!(config.max_pixels.get(), 5678);
        assert_eq!(config.concurrency.get(), 2);

        let config = parse(&[args[0], args[1], "--photo-upload-max-size", "4321"])
            .unwrap()
            .api
            .photo_upload;

        assert_eq!(config.max_size.get(), 4321);
        assert_eq!(config.max_pixels.get(), 5678);
        assert_eq!(config.concurrency.get(), 2);
    }

    #[test]
    fn photo_upload_concurrency_rejects_zero() {
        assert!(parse(&["--photo-upload-concurrency", "0"]).is_err());
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
}
