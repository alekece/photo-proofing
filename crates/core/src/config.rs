use clap::Args;
use clap_config_fallback::ConfigArgs;

#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct ServerConfig {
    #[arg(long, env = "EG_HOST", default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long, env = "EG_PORT", default_value_t = 8080)]
    pub port: u16,
}

#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct LogConfig {
    #[arg(long, env = "EG_LOG_LEVEL", default_value = "info")]
    pub log_level: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use clap_config_fallback::ConfigParser;
    use std::path::PathBuf;

    #[derive(Parser, ConfigParser)]
    struct TestCli {
        #[arg(long, env = "EG_CONFIG")]
        #[config(path)]
        config: Option<PathBuf>,
        #[command(flatten)]
        server: ServerConfig,
        #[command(flatten)]
        log: LogConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(std::iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_apply_without_config_file() {
        let cli = parse(&[]).unwrap();
        assert_eq!(cli.server.host, "127.0.0.1");
        assert_eq!(cli.server.port, 8080);
        assert_eq!(cli.log.log_level, "info");
    }

    #[test]
    fn config_file_overrides_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        std::fs::write(&path, "port = 9000\n").unwrap();
        let cli = parse(&["--config", path.to_str().unwrap()]).unwrap();
        assert_eq!(cli.server.port, 9000);
    }

    #[test]
    fn cli_overrides_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        std::fs::write(&path, "port = 9000\n").unwrap();
        let cli = parse(&["--config", path.to_str().unwrap(), "--port", "9200"]).unwrap();
        assert_eq!(cli.server.port, 9200);
    }

    #[test]
    fn invalid_port_is_rejected() {
        assert!(parse(&["--port", "abc"]).is_err());
        assert!(parse(&["--port", "70000"]).is_err());
    }
}
