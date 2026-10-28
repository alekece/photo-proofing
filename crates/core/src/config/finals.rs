use clap::Args;
use clap_config_fallback::ConfigArgs;

/// Limits applied to final renditions before they are accepted.
#[derive(Args, ConfigArgs, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinalsConfig {
    /// Maximum size of a final file, in bytes (default: 100 MiB).
    #[arg(long, env = "EG_FINALS_MAX_BYTES", default_value_t = 100 * 1024 * 1024)]
    pub finals_max_bytes: u64,
    /// Maximum image area of a final, in megapixels (1 MP = 1,000,000 pixels).
    #[arg(long, env = "EG_FINALS_MAX_MEGAPIXELS", default_value_t = 100)]
    pub finals_max_megapixels: u64,
}

impl FinalsConfig {
    pub fn max_pixels(&self) -> u64 {
        self.finals_max_megapixels.saturating_mul(1_000_000)
    }
}

impl Default for FinalsConfig {
    fn default() -> Self {
        Self {
            finals_max_bytes: 100 * 1024 * 1024,
            finals_max_megapixels: 100,
        }
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
        finals: FinalsConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_are_100_mib_and_100_megapixels() {
        let cli = parse(&[]).expect("defaults should parse");

        assert_eq!(cli.finals, FinalsConfig::default());
        assert_eq!(cli.finals.max_pixels(), 100_000_000);
    }

    #[test]
    fn config_file_and_cli_override_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("c.toml");
        fs::write(&path, "finals_max_bytes = 10\nfinals_max_megapixels = 5\n").expect("write");

        let cli = parse(&[
            "--config",
            path.to_str().expect("utf-8 path"),
            "--finals-max-megapixels",
            "7",
        ])
        .expect("parse");

        assert_eq!(cli.finals.finals_max_bytes, 10);
        assert_eq!(cli.finals.finals_max_megapixels, 7);
    }

    #[test]
    fn non_numeric_limit_is_rejected() {
        assert!(parse(&["--finals-max-bytes", "big"]).is_err());
    }
}
