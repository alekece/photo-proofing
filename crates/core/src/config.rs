use std::{
    ffi::OsStr,
    fmt,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use clap::{
    Arg, Args, Command, Error as ClapError, ValueEnum, builder::TypedValueParser, error::ErrorKind,
};
use clap_config_fallback::ConfigArgs;
use derive_more::Display as DeriveDisplay;
use serde::{Deserialize, Serialize};
use strum::Display;
use thiserror::Error;
use tracing::level_filters::LevelFilter;
use url::Url;

use crate::{
    email::Email,
    time::BoundedDuration,
    types::access_code::{AccessCodeCipher, EncryptionKey},
};

/// Default SMTP submission port (`STARTTLS`).
pub const DEFAULT_SMTP_PORT: u16 = 587;

/// Default maximum recipients accepted in one gallery invitation send.
pub const DEFAULT_MAX_RECIPIENTS_PER_SEND: NonZeroUsize = NonZeroUsize::new(50).unwrap();

/// Largest original, in pixels, that the API accepts and the worker renders.
pub const DEFAULT_MAX_PIXELS: NonZeroU64 = NonZeroU64::new(100_000_000).unwrap();

/// Default cap on the originals of one selection ZIP: 2 GiB.
pub const DEFAULT_EXPORT_MAX_SIZE: NonZeroU64 = NonZeroU64::new(2 << 30).unwrap();

/// Default time a finished selection ZIP stays available.
pub const DEFAULT_EXPORT_RETENTION: Duration = Duration::from_secs(15 * 60);

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
        default_value = "15m"
    )]
    pub retention: ExportRetention,
}

/// Optional encryption key for per-gallery 6-digit access codes (`[gallery_password]`).
/// Deliberately omits `Debug` because it holds secret key material.
#[derive(Args, ConfigArgs, Default, Clone)]
pub struct GalleryPasswordConfig {
    /// 256-bit key (64 hex digits or 43 unpadded base64url characters) encrypting gallery access
    /// codes at rest and signing client unlock cookies.
    #[arg(
        long = "gallery-password-key",
        env = "PP_GALLERY_PASSWORD_KEY",
        hide_env_values = true,
        value_parser = EncryptionKey::value_parser()
    )]
    #[config(value_format = EncryptionKey::encode_base64url)]
    pub encryption_key: Option<EncryptionKey>,
}

impl GalleryPasswordConfig {
    /// Builds an [`AccessCodeCipher`] when `encryption_key` is configured.
    pub fn cipher(&self) -> Option<AccessCodeCipher> {
        self.encryption_key.as_ref().map(AccessCodeCipher::new)
    }
}

/// Transport encryption mode for outbound SMTP connections.
#[derive(
    ValueEnum, Display, Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum SmtpTlsMode {
    /// Implicit TLS (`SMTPS`, typically port `465`).
    Tls,
    /// Opportunistic-upgrade required `STARTTLS` (typically port `587`).
    #[default]
    Starttls,
    /// Unencrypted SMTP (for local development tools such as Mailpit or loopback relays).
    None,
}

/// Empty or NUL-bearing [`SmtpPassword`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("SMTP password must not be empty")]
pub struct ParseSmtpPasswordError;

/// Incomplete or invalid `[smtp]` operator configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SmtpConfigError {
    #[error("[smtp] requires both host and from_email when SMTP settings are provided")]
    MissingRequiredField,
    #[error("[smtp] host must be a non-empty hostname without whitespace or control characters")]
    InvalidHost,
    #[error("[smtp] username and password must both be set when SMTP authentication is used")]
    IncompleteCredentials,
    #[error("[smtp] from_name must not contain control characters")]
    InvalidFromName,
}

/// SMTP authentication secret with redacted [`fmt::Debug`] and hidden CLI/env error output.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SmtpPassword(String);

impl SmtpPassword {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub const fn value_parser() -> SmtpPasswordValueParser {
        SmtpPasswordValueParser
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Debug for SmtpPassword {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SmtpPassword(..)")
    }
}

impl FromStr for SmtpPassword {
    type Err = ParseSmtpPasswordError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() || value.contains('\0') {
            return Err(ParseSmtpPasswordError);
        }

        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for SmtpPassword {
    type Error = ParseSmtpPasswordError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<SmtpPassword> for String {
    fn from(password: SmtpPassword) -> Self {
        password.0
    }
}

/// Clap parser for [`SmtpPassword`] that omits [`clap::error::ContextKind::InvalidValue`] so
/// `PP_SMTP_PASSWORD` values are never echoed on parse errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmtpPasswordValueParser;

impl TypedValueParser for SmtpPasswordValueParser {
    type Value = SmtpPassword;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, ClapError> {
        match value
            .to_str()
            .ok_or(ParseSmtpPasswordError)
            .and_then(SmtpPassword::from_str)
        {
            Ok(password) => Ok(password),
            Err(error) => {
                let flag = arg.map_or_else(|| "--smtp-password".to_owned(), ToString::to_string);

                Err(cmd.clone().error(
                    ErrorKind::ValueValidation,
                    format!("invalid value for '{flag}': {error}"),
                ))
            }
        }
    }
}

/// Validated outbound SMTP configuration once `[smtp]` is enabled.
#[derive(Clone, PartialEq, Eq)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub tls: SmtpTlsMode,
    pub credentials: Option<(String, SmtpPassword)>,
    pub from_email: Email,
    pub from_name: Option<String>,
    pub max_recipients_per_send: NonZeroUsize,
}

impl fmt::Debug for SmtpSettings {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SmtpSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("tls", &self.tls)
            .field("authenticated", &self.credentials.is_some())
            .field("from_email", &self.from_email)
            .field("from_name", &self.from_name)
            .field("max_recipients_per_send", &self.max_recipients_per_send)
            .finish()
    }
}

/// Optional outbound SMTP configuration (`[smtp]`) shared by the API and the worker.
/// Deliberately omits `Debug` because it may carry [`SmtpPassword`].
#[derive(Args, ConfigArgs, Clone)]
pub struct SmtpConfig {
    /// Outbound SMTP server hostname; omit `[smtp]` when email invitations are not used.
    #[arg(id = "smtp_host", long = "smtp-host", env = "PP_SMTP_HOST")]
    pub host: Option<String>,
    /// Outbound SMTP port (`587` for `starttls`, `465` for `tls`, `1025` for local relays).
    #[arg(
        id = "smtp_port",
        long = "smtp-port",
        env = "PP_SMTP_PORT",
        default_value_t = DEFAULT_SMTP_PORT
    )]
    pub port: u16,
    /// Transport encryption mode (`tls`, `starttls`, or `none`).
    #[arg(
        id = "smtp_tls",
        long = "smtp-tls",
        env = "PP_SMTP_TLS",
        value_enum,
        default_value_t = SmtpTlsMode::Starttls
    )]
    pub tls: SmtpTlsMode,
    /// Optional SMTP authentication username; requires `password` when set.
    #[arg(id = "smtp_username", long = "smtp-username", env = "PP_SMTP_USERNAME")]
    pub username: Option<String>,
    /// Optional SMTP authentication password; inject `PP_SMTP_PASSWORD` from a secret manager.
    #[arg(
        id = "smtp_password",
        long = "smtp-password",
        env = "PP_SMTP_PASSWORD",
        hide_env_values = true,
        value_parser = SmtpPassword::value_parser()
    )]
    #[config(value_format = SmtpPassword::into_inner)]
    pub password: Option<SmtpPassword>,
    /// Envelope and `From` email address for outbound studio emails.
    #[arg(
        id = "smtp_from_email",
        long = "smtp-from-email",
        env = "PP_SMTP_FROM_EMAIL"
    )]
    pub from_email: Option<Email>,
    /// Optional fallback `From` display name when the workspace has no `studio_name`.
    #[arg(
        id = "smtp_from_name",
        long = "smtp-from-name",
        env = "PP_SMTP_FROM_NAME"
    )]
    pub from_name: Option<String>,
    /// Maximum unique recipients (`to` + `bcc`) accepted in one invitation send.
    #[arg(
        id = "smtp_max_recipients_per_send",
        long = "smtp-max-recipients-per-send",
        env = "PP_SMTP_MAX_RECIPIENTS_PER_SEND",
        default_value_t = DEFAULT_MAX_RECIPIENTS_PER_SEND
    )]
    pub max_recipients_per_send: NonZeroUsize,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: None,
            port: DEFAULT_SMTP_PORT,
            tls: SmtpTlsMode::Starttls,
            username: None,
            password: None,
            from_email: None,
            from_name: None,
            max_recipients_per_send: DEFAULT_MAX_RECIPIENTS_PER_SEND,
        }
    }
}

impl SmtpConfig {
    /// Validates `[smtp]` into [`Some(SmtpSettings)`](SmtpSettings) when configured, [`None`] when
    /// omitted, or [`SmtpConfigError`] when only half-configured.
    pub fn resolve(&self) -> Result<Option<SmtpSettings>, SmtpConfigError> {
        let any_provided = self.host.is_some()
            || self.from_email.is_some()
            || self.username.is_some()
            || self.password.is_some()
            || self.from_name.is_some();

        if !any_provided {
            return Ok(None);
        }

        let Some(raw_host) = self.host.as_deref() else {
            return Err(SmtpConfigError::MissingRequiredField);
        };
        let host = raw_host.trim();

        if host.is_empty() || host.chars().any(|ch| ch.is_whitespace() || ch.is_control()) {
            return Err(SmtpConfigError::InvalidHost);
        }

        let Some(from_email) = self.from_email.clone() else {
            return Err(SmtpConfigError::MissingRequiredField);
        };

        let credentials = match (&self.username, &self.password) {
            (None, None) => None,
            (Some(raw_user), Some(password)) => {
                let user = raw_user.trim();

                if user.is_empty() || user.chars().any(char::is_control) {
                    return Err(SmtpConfigError::IncompleteCredentials);
                }

                Some((user.to_owned(), password.clone()))
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(SmtpConfigError::IncompleteCredentials);
            }
        };

        let from_name = match self.from_name.as_deref() {
            None => None,
            Some(raw_name) => {
                if raw_name.chars().any(char::is_control) {
                    return Err(SmtpConfigError::InvalidFromName);
                }

                let trimmed = raw_name.trim();

                (!trimmed.is_empty()).then(|| trimmed.to_owned())
            }
        };

        Ok(Some(SmtpSettings {
            host: host.to_owned(),
            port: self.port,
            tls: self.tls,
            credentials,
            from_email,
            from_name,
            max_recipients_per_send: self.max_recipients_per_send,
        }))
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
        log: LogConfig,
        #[command(flatten)]
        #[config(no_flatten)]
        database: DatabaseConfig,
        #[command(flatten)]
        #[config(no_flatten)]
        photo: PhotoConfig,
        #[command(flatten)]
        #[config(no_flatten)]
        smtp: SmtpConfig,
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

    #[test]
    fn smtp_config_is_optional_and_rejects_incomplete_sections() {
        let default_smtp = SmtpConfig::default();

        assert!(default_smtp.resolve().unwrap().is_none());

        let partial_host_only = SmtpConfig {
            host: Some("smtp.example.test".to_owned()),
            ..SmtpConfig::default()
        };

        assert!(partial_host_only.resolve().is_err());

        let partial_user_without_pass = SmtpConfig {
            host: Some("smtp.example.test".to_owned()),
            from_email: Some("studio@example.test".parse().unwrap()),
            username: Some("apikey".to_owned()),
            password: None,
            ..SmtpConfig::default()
        };

        assert!(partial_user_without_pass.resolve().is_err());

        let password: SmtpPassword = "super-secret-smtp-pass".parse().unwrap();

        assert_eq!(format!("{password:?}"), "SmtpPassword(..)");
        assert!(!format!("{password:?}").contains("super-secret-smtp-pass"));

        let complete = SmtpConfig {
            host: Some(" smtp.example.test ".to_owned()),
            port: 465,
            tls: SmtpTlsMode::Tls,
            username: Some("apikey".to_owned()),
            password: Some(password),
            from_email: Some("studio@example.test".parse().unwrap()),
            from_name: Some("  Atelier Lumière  ".to_owned()),
            max_recipients_per_send: std::num::NonZeroUsize::new(25).unwrap(),
        };
        let resolved = complete.resolve().unwrap().unwrap();

        assert_eq!(resolved.host, "smtp.example.test");
        assert_eq!(resolved.port, 465);
        assert_eq!(resolved.tls, SmtpTlsMode::Tls);
        assert_eq!(resolved.from_email.as_str(), "studio@example.test");
        assert_eq!(resolved.from_name.as_deref(), Some("Atelier Lumière"));
        assert_eq!(resolved.max_recipients_per_send.get(), 25);
        assert_eq!(
            resolved
                .credentials
                .as_ref()
                .map(|(u, p)| (u.as_str(), p.as_str())),
            Some(("apikey", "super-secret-smtp-pass"))
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.toml");
        fs::write(
            &path,
            "[smtp]\nhost = 'smtp.example.test'\nport = 2525\ntls = 'starttls'\nfrom_email = 'studio@example.test'\nmax_recipients_per_send = 30\n",
        )
        .unwrap();

        let cli = parse(&["--config", path.to_str().unwrap()]).unwrap();
        let from_file = cli.smtp.resolve().unwrap().unwrap();

        assert_eq!(from_file.host, "smtp.example.test");
        assert_eq!(from_file.port, 2525);
        assert_eq!(from_file.tls, SmtpTlsMode::Starttls);
        assert_eq!(from_file.from_email.as_str(), "studio@example.test");
        assert_eq!(from_file.max_recipients_per_send.get(), 30);
    }
}
