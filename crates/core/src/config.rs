use clap::{Args, ValueEnum};
use clap_config_fallback::ConfigArgs;
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::level_filters::LevelFilter;
use url::Url;

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

#[cfg(test)]
mod tests {
    use std::{fs, iter, path::PathBuf};

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
        log: LogConfig,
        #[command(flatten)]
        #[config(no_flatten)]
        database: DatabaseConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn log_level_defaults_to_info() {
        assert_eq!(parse(&[]).unwrap().log.log_level, LogLevel::Info);
    }

    #[test]
    fn database_url_is_validated_at_parse_time() {
        assert!(parse(&["--database-url", "not a url"]).is_err());
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
}
