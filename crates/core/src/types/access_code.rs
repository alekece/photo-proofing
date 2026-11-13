use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Length of a gallery access code in ASCII digits.
pub const ACCESS_CODE_DIGITS: usize = 6;

/// The input is not exactly 6 ASCII digits (`000000`–`999999`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("access code must be 6 ASCII digits")]
pub struct ParseAccessCodeError;

/// Six-digit numeric code (`000000`–`999999`) gating client access to a shared gallery.
///
/// [`Debug`](fmt::Debug) redacts the digits so credentials never leak into logs or spans.
///
/// ```
/// use pp_core::types::access_code::GalleryAccessCode;
///
/// let code: GalleryAccessCode = "042819".parse().unwrap();
/// assert_eq!(code.as_str(), "042819");
/// assert_eq!(format!("{code:?}"), "GalleryAccessCode(..)");
/// assert!("12345".parse::<GalleryAccessCode>().is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GalleryAccessCode(String);

impl GalleryAccessCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for GalleryAccessCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GalleryAccessCode(..)")
    }
}

impl FromStr for GalleryAccessCode {
    type Err = ParseAccessCodeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != ACCESS_CODE_DIGITS || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(ParseAccessCodeError);
        }

        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for GalleryAccessCode {
    type Error = ParseAccessCodeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<GalleryAccessCode> for String {
    fn from(code: GalleryAccessCode) -> Self {
        code.0
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_only_six_ascii_digits() {
        for valid in ["000000", "042819", "123456", "999999"] {
            let parsed: GalleryAccessCode = valid.parse().expect(valid);

            assert_eq!(parsed.as_str(), valid);
            assert_eq!(parsed.as_bytes(), valid.as_bytes());
            assert_eq!(serde_json::to_value(&parsed).unwrap(), json!(valid));
            assert_eq!(
                serde_json::from_value::<GalleryAccessCode>(json!(valid)).unwrap(),
                parsed
            );
        }

        for invalid in [
            "",
            "0",
            "12345",
            "1234567",
            " 123456",
            "123456 ",
            "12 345",
            "+12345",
            "-12345",
            "abcdef",
            "12345a",
            "١٢٣٤٥٦",
        ] {
            let err = invalid.parse::<GalleryAccessCode>().unwrap_err();

            if !invalid.is_empty() {
                assert!(!err.to_string().contains(invalid), "{invalid:?}");
            }
            assert!(
                serde_json::from_value::<GalleryAccessCode>(json!(invalid)).is_err(),
                "{invalid:?}"
            );
        }
    }

    #[test]
    fn debug_redacts_the_digits() {
        let code: GalleryAccessCode = "042819".parse().unwrap();

        assert_eq!(format!("{code:?}"), "GalleryAccessCode(..)");
        assert!(!format!("{code:?}").contains("042819"));
    }
}
