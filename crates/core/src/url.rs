use std::str::FromStr;

use derive_more::Display;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
#[error("application origin must be an exact HTTP(S) origin without a path")]
pub struct ParseOriginError;

/// The browser-visible origin of the app, in canonical form (`scheme://host[:port]`, no
/// trailing slash).
///
/// Parsing accepts only that exact form, so a path, query, fragment, credentials or a
/// non-HTTP(S) scheme can never reach the API's byte-for-byte comparison against the
/// `Origin` request header.
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Origin(String);

impl Origin {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Origin {
    type Err = ParseOriginError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let url = Url::parse(value).map_err(|_| ParseOriginError)?;

        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
            || url.origin().ascii_serialization() != value
        {
            return Err(ParseOriginError);
        }

        Ok(Self(value.to_owned()))
    }
}

impl TryFrom<String> for Origin {
    type Error = ParseOriginError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Origin> for String {
    fn from(origin: Origin) -> Self {
        origin.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_canonical_origins() {
        for value in ["http://localhost:5173", "https://photos.example.test"] {
            assert_eq!(value.parse::<Origin>().unwrap().as_str(), value);
        }
    }

    #[test]
    fn rejects_everything_but_an_exact_origin() {
        for value in [
            "null",
            "ftp://localhost",
            "http://localhost/path",
            "http://localhost/",
            "http://localhost?q=1",
            "http://localhost#f",
            "http://user:pass@localhost",
            "HTTP://localhost",
        ] {
            assert!(value.parse::<Origin>().is_err(), "{value}");
        }
    }
}
