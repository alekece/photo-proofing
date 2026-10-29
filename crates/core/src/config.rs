use clap::{Args, ValueEnum};
use clap_config_fallback::ConfigArgs;
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::level_filters::LevelFilter;

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
        server: ServerConfig,
        #[command(flatten)]
        log: LogConfig,
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
}
