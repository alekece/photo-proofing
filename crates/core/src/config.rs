use clap::{Args, ValueEnum};
use clap_config_fallback::ConfigArgs;
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::level_filters::LevelFilter;
use url::Url;

use crate::{time::NonZeroDuration, url::Origin};

/// Network binding shared by every process that listens for connections.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct ServerConfig {
    #[arg(long, env = "PP_HOST", default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, env = "PP_PORT", default_value_t = 8080)]
    pub port: u16,
}

#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct LogConfig {
    #[arg(long, env = "PP_LOG_LEVEL", value_enum, default_value_t = LogLevel::Info)]
    pub log_level: LogLevel,
}

/// Typed so that a misspelled level fails at startup instead of silently
/// disabling logs.
#[derive(ValueEnum, Display, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Self::ERROR,
            LogLevel::Warn => Self::WARN,
            LogLevel::Info => Self::INFO,
            LogLevel::Debug => Self::DEBUG,
            LogLevel::Trace => Self::TRACE,
        }
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
    pub idle_timeout: NonZeroDuration,
}

/// Database connection settings. Deliberately not Debug: URLs may contain passwords.
#[derive(Args, ConfigArgs, Clone)]
pub struct DatabaseConfig {
    #[arg(
        long = "database-url",
        env = "PP_DATABASE_URL",
        default_value = "postgres://pp:pp-local-only@127.0.0.1:5432/pp"
    )]
    pub url: Url,
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

#[cfg(test)]
mod tests {
    use std::{env, fs, iter, path::PathBuf, process::Command, time::Duration};

    use clap::Parser;
    use clap_config_fallback::ConfigParser;
    use tracing::level_filters::LevelFilter;

    use super::*;

    #[derive(Parser, ConfigParser)]
    struct TestCli {
        #[arg(long, env = "PP_CONFIG")]
        #[config(path)]
        config: Option<PathBuf>,
        #[command(flatten)]
        server: ServerConfig,
        #[command(flatten)]
        log: LogConfig,
        #[command(flatten)]
        #[config(no_flatten)]
        session: SessionConfig,
        #[command(flatten)]
        #[config(no_flatten)]
        auth: AuthConfig,
        #[command(flatten)]
        #[config(no_flatten)]
        database: DatabaseConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_apply_without_config_file() {
        let cli = parse(&[]).unwrap();
        assert_eq!(cli.server.host, "127.0.0.1");
        assert_eq!(cli.server.port, 8080);
        assert_eq!(cli.log.log_level, LogLevel::Info);
    }

    #[test]
    fn config_file_overrides_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "port = 9000\n").unwrap();
        let cli = parse(&["--config", path.to_str().unwrap()]).unwrap();
        assert_eq!(cli.server.port, 9000);
    }

    #[test]
    fn cli_overrides_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "port = 9000\n").unwrap();
        let cli = parse(&["--config", path.to_str().unwrap(), "--port", "9200"]).unwrap();
        assert_eq!(cli.server.port, 9200);
    }

    #[test]
    fn invalid_port_is_rejected() {
        assert!(parse(&["--port", "abc"]).is_err());
        assert!(parse(&["--port", "70000"]).is_err());
    }

    #[test]
    fn log_level_is_read_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "log_level = \"debug\"\n").unwrap();

        let cli = parse(&["--config", path.to_str().unwrap()]).unwrap();

        assert_eq!(cli.log.log_level, LogLevel::Debug);
    }

    #[test]
    fn unknown_log_level_is_rejected() {
        assert!(parse(&["--log-level", "loud"]).is_err());
    }

    #[test]
    fn log_level_converts_to_level_filter() {
        assert_eq!(LevelFilter::from(LogLevel::Warn), LevelFilter::WARN);
    }

    #[test]
    fn session_and_auth_defaults_apply() {
        let cli = parse(&[]).unwrap();

        assert_eq!(
            cli.session.idle_timeout.get(),
            Duration::from_secs(30 * 86400)
        );
        assert_eq!(cli.auth.app_origin.as_str(), "http://localhost:5173");
    }

    #[test]
    fn sectioned_human_durations_and_cli_override() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "[session]\nidle_timeout = '1h 30m'\n").unwrap();
        let args = ["--config", path.to_str().unwrap()];

        let cli = parse(&args).unwrap();

        assert_eq!(cli.session.idle_timeout.get(), Duration::from_secs(5400));

        let cli = parse(&[args[0], args[1], "--session-idle-timeout", "2h"]).unwrap();

        assert_eq!(cli.session.idle_timeout.get(), Duration::from_secs(7200));
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
    fn environment_precedence_is_process_isolated() {
        if env::var_os("PP_TEST_CONFIG_CHILD").is_some() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("c.toml");
            fs::write(&path, "[session]\nidle_timeout = '1h'\n").unwrap();
            let args = ["--config", path.to_str().unwrap()];

            let cli = parse(&args).unwrap();

            assert_eq!(cli.session.idle_timeout.get(), Duration::from_secs(7200));

            let cli = parse(&[args[0], args[1], "--session-idle-timeout", "3h"]).unwrap();

            assert_eq!(cli.session.idle_timeout.get(), Duration::from_secs(10800));

            return;
        }

        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "config::tests::environment_precedence_is_process_isolated",
            ])
            .env("PP_TEST_CONFIG_CHILD", "1")
            .env("PP_SESSION_IDLE_TIMEOUT", "2h")
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
}
