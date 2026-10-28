use clap::Args;
use clap_config_fallback::ConfigArgs;

/// Upload limits enforced before any original is accepted.
///
/// Zero is rejected at parse time: a zero limit would silently refuse every
/// upload.
#[derive(Args, ConfigArgs, Debug, Clone)]
pub struct UploadConfig {
    /// Maximum number of files in one upload request.
    #[arg(
        long,
        env = "EG_UPLOAD_MAX_FILES",
        default_value_t = 100,
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub upload_max_files: u32,
    /// Maximum size of one file in bytes (default 100 MiB).
    #[arg(
        long,
        env = "EG_UPLOAD_MAX_FILE_BYTES",
        default_value_t = 100 * 1024 * 1024,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    pub upload_max_file_bytes: u64,
    /// Maximum JPEG resolution in megapixels (1 MP = 1,000,000 pixels).
    #[arg(
        long,
        env = "EG_UPLOAD_MAX_MEGAPIXELS",
        default_value_t = 100,
        value_parser = clap::value_parser!(u32).range(1..)
    )]
    pub upload_max_megapixels: u32,
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
        upload: UploadConfig,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        TestCli::try_parse_with_config_from(iter::once("test").chain(args.iter().copied()))
    }

    #[test]
    fn defaults_are_100_files_100_mib_100_megapixels() {
        let cli = parse(&[]).expect("defaults parse");

        assert_eq!(cli.upload.upload_max_files, 100);
        assert_eq!(cli.upload.upload_max_file_bytes, 100 * 1024 * 1024);
        assert_eq!(cli.upload.upload_max_megapixels, 100);
    }

    #[test]
    fn config_file_sets_limits() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("c.toml");
        fs::write(
            &path,
            "upload_max_files = 5\nupload_max_file_bytes = 1024\nupload_max_megapixels = 12\n",
        )
        .expect("write config");

        let cli = parse(&["--config", path.to_str().expect("utf-8 path")]).expect("parse");

        assert_eq!(cli.upload.upload_max_files, 5);
        assert_eq!(cli.upload.upload_max_file_bytes, 1024);
        assert_eq!(cli.upload.upload_max_megapixels, 12);
    }

    #[test]
    fn cli_overrides_config_file() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("c.toml");
        fs::write(&path, "upload_max_files = 5\n").expect("write config");

        let cli = parse(&[
            "--config",
            path.to_str().expect("utf-8 path"),
            "--upload-max-files",
            "7",
        ])
        .expect("parse");

        assert_eq!(cli.upload.upload_max_files, 7);
    }

    #[test]
    fn zero_limits_are_rejected() {
        assert!(parse(&["--upload-max-files", "0"]).is_err());
        assert!(parse(&["--upload-max-file-bytes", "0"]).is_err());
        assert!(parse(&["--upload-max-megapixels", "0"]).is_err());
    }
}
