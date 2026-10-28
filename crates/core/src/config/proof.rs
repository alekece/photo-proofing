use clap::Args;
use clap_config_fallback::ConfigArgs;

/// Limits applied when generating client-facing proof previews.
///
/// The source limits reject hostile uploads (decompression bombs) before any
/// pixel buffer is allocated; memory per job is roughly 10 bytes per source
/// pixel, so size them against worker concurrency.
#[derive(Args, ConfigArgs, Debug, Clone, PartialEq, Eq)]
pub struct ProofConfig {
    /// Maximum length of the proof's long edge, in pixels.
    #[arg(long, env = "EG_PROOF_MAX_EDGE", default_value_t = 2048, value_parser = clap::value_parser!(u32).range(64..=16384))]
    pub proof_max_edge: u32,
    /// JPEG quality of the proof, from 1 (worst) to 100 (best).
    #[arg(long, env = "EG_PROOF_QUALITY", default_value_t = 80, value_parser = clap::value_parser!(u8).range(1..=100))]
    pub proof_quality: u8,
    /// Largest accepted source image, in pixels (width x height).
    #[arg(long, env = "EG_PROOF_MAX_SOURCE_PIXELS", default_value_t = 50_000_000, value_parser = clap::value_parser!(u64).range(1..))]
    pub proof_max_source_pixels: u64,
    /// Largest accepted source file, in bytes.
    #[arg(long, env = "EG_PROOF_MAX_SOURCE_BYTES", default_value_t = 128 * 1024 * 1024, value_parser = clap::value_parser!(u64).range(1..))]
    pub proof_max_source_bytes: u64,
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
        proof: ProofConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_match_product_decision() {
        let proof = parse(&[]).expect("defaults parse").proof;

        assert_eq!(proof.proof_max_edge, 2048);
        assert_eq!(proof.proof_quality, 80);
    }

    #[test]
    fn config_file_overrides_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("c.toml");
        fs::write(&path, "proof_max_edge = 1024\nproof_quality = 70\n").expect("write config");

        let proof = parse(&["--config", path.to_str().expect("utf8 path")])
            .expect("config parses")
            .proof;

        assert_eq!(proof.proof_max_edge, 1024);
        assert_eq!(proof.proof_quality, 70);
    }

    #[test]
    fn out_of_range_values_are_rejected() {
        assert!(parse(&["--proof-quality", "0"]).is_err());
        assert!(parse(&["--proof-quality", "101"]).is_err());
        assert!(parse(&["--proof-max-edge", "10"]).is_err());
        assert!(parse(&["--proof-max-source-pixels", "0"]).is_err());
    }
}
