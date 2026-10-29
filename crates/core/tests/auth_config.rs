use std::{fs, time::Duration};

use clap::Parser;
use clap_config_fallback::ConfigParser;
use pp_core::config::{AuthConfig, DatabaseConfig, SessionConfig};
use tempfile::tempdir;

#[derive(Parser, ConfigParser)]
struct Cli {
    #[arg(long)]
    #[config(path)]
    config: Option<String>,
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

#[test]
fn defaults_and_sectioned_human_durations() {
    let defaults = Cli::try_parse_with_config_from(["test"]).unwrap();

    assert_eq!(
        defaults.session.idle_timeout,
        Duration::from_secs(30 * 86400)
    );
    defaults.session.validate().unwrap();
    defaults.auth.validate().unwrap();

    let dir = tempdir().unwrap();
    let file = dir.path().join("config.toml");
    fs::write(&file, "[session]\nidle_timeout = '1h 30m'\n").unwrap();
    let args = ["test", "--config", file.to_str().unwrap()];
    let parsed = Cli::try_parse_with_config_from(args).unwrap();

    assert_eq!(parsed.session.idle_timeout, Duration::from_secs(5400));

    let overridden =
        Cli::try_parse_with_config_from(args.into_iter().chain(["--session-idle-timeout", "2h"]))
            .unwrap();

    assert_eq!(overridden.session.idle_timeout, Duration::from_secs(7200));
}

#[test]
fn invalid_durations_and_origins_fail() {
    for value in ["-1s", "nonsense"] {
        assert!(
            Cli::try_parse_with_config_from(["test", "--session-idle-timeout", value]).is_err()
        );
    }

    for value in ["0s", "500ms", "999999999999999s"] {
        let parsed = Cli::try_parse_with_config_from(["test", "--session-idle-timeout", value]);

        assert!(parsed.is_err() || parsed.unwrap().session.validate().is_err());
    }

    for origin in [
        "null",
        "ftp://localhost",
        "http://localhost/path",
        "http://user:pass@localhost",
    ] {
        let parsed = Cli::try_parse_with_config_from(["test", "--app-origin", origin]).unwrap();

        assert!(parsed.auth.validate().is_err(), "{origin}");
    }
}

#[test]
fn environment_precedence_is_process_isolated() {
    if std::env::var_os("PP63_CONFIG_CHILD").is_some() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("config.toml");
        fs::write(&file, "[session]\nidle_timeout = '1h'\n").unwrap();
        let args = ["test", "--config", file.to_str().unwrap()];
        let parsed = Cli::try_parse_with_config_from(args).unwrap();

        assert_eq!(parsed.session.idle_timeout, Duration::from_secs(7200));

        let parsed = Cli::try_parse_with_config_from(
            args.into_iter().chain(["--session-idle-timeout", "3h"]),
        )
        .unwrap();

        assert_eq!(parsed.session.idle_timeout, Duration::from_secs(10800));

        return;
    }

    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "environment_precedence_is_process_isolated"])
        .env("PP63_CONFIG_CHILD", "1")
        .env("PP_SESSION_IDLE_TIMEOUT", "2h")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}
