use std::{
    num::NonZeroU64,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use clap::{Args, ValueEnum};
use clap_config_fallback::ConfigArgs;
use derive_more::Display as DeriveDisplay;
use serde::{Deserialize, Serialize};
use strum::Display;
use thiserror::Error;
use tracing::level_filters::LevelFilter;
use url::Url;

use crate::time::BoundedDuration;

/// Largest original, in pixels, that the API accepts and the worker renders.
pub const DEFAULT_MAX_PIXELS: NonZeroU64 = NonZeroU64::new(100_000_000).unwrap();

/// Default cap on the originals of one selection ZIP: 2 GiB.
pub const DEFAULT_EXPORT_MAX_SIZE: NonZeroU64 = NonZeroU64::new(2 << 30).unwrap();

/// Default time a finished selection ZIP stays available.
pub const DEFAULT_EXPORT_RETENTION: Duration = Duration::from_secs(24 * 60 * 60);

/// Longest configurable ZIP retention: 30 days.
const MAX_EXPORT_RETENTION_MILLIS: u64 = 30 * 24 * 60 * 60 * 1000;

/// How long a finished ZIP stays available, from one minute up to 30 days.
pub type ExportRetention = BoundedDuration<60_000, MAX_EXPORT_RETENTION_MILLIS>;

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

/// The photo directory setting is empty.
#[derive(Debug, Error)]
#[error("photo directory must not be empty")]
pub struct ParsePhotoDirError;

/// Directory holding the private photo files, parsed once so an empty value cannot reach the
/// storage layer. Whether it exists and is writable is checked when storage is opened.
#[derive(Debug, Clone, PartialEq, Eq, DeriveDisplay, Serialize, Deserialize)]
#[display("{}", _0.display())]
#[serde(try_from = "PathBuf", into = "PathBuf")]
pub struct PhotoDir(PathBuf);

impl PhotoDir {
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl TryFrom<PathBuf> for PhotoDir {
    type Error = ParsePhotoDirError;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        if path.as_os_str().is_empty() {
            return Err(ParsePhotoDirError);
        }

        Ok(Self(path))
    }
}

impl FromStr for PhotoDir {
    type Err = ParsePhotoDirError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        PathBuf::from(value).try_into()
    }
}

impl From<PhotoDir> for PathBuf {
    fn from(dir: PhotoDir) -> Self {
        dir.0
    }
}

/// Where the API and the worker keep photo files; both must point at the same directory.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct PhotoConfig {
    #[arg(
        long = "photo-dir",
        env = "PP_PHOTO_DIR",
        default_value = "./data/photos"
    )]
    pub dir: PhotoDir,
}

/// Limits on the ZIP of a gallery's selection; the API and the worker must agree on them.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct ExportConfig {
    /// Selections whose originals together exceed this many bytes are refused.
    #[arg(
        long = "export-max-size",
        env = "PP_EXPORT_MAX_SIZE",
        default_value_t = DEFAULT_EXPORT_MAX_SIZE
    )]
    pub size_limit: NonZeroU64,
    /// A ZIP is unavailable and deleted once it is older than this.
    #[arg(
        long = "export-retention",
        env = "PP_EXPORT_RETENTION",
        default_value = "24h"
    )]
    pub retention: ExportRetention,
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
        #[command(flatten)]
        #[config(no_flatten)]
        photo: PhotoConfig,
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

    #[test]
    fn photo_dir_has_a_default_and_rejects_empty_values() {
        assert_eq!(
            parse(&[]).unwrap().photo.dir.as_path(),
            Path::new("./data/photos")
        );
        assert!(parse(&["--photo-dir", ""]).is_err());
    }

    #[test]
    fn photo_dir_is_read_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(&path, "[photo]\ndir = \"/srv/photos\"\n").unwrap();

        let cli = parse(&["--config", path.to_str().unwrap()]).unwrap();

        assert_eq!(cli.photo.dir.as_path(), Path::new("/srv/photos"));
    }
}
