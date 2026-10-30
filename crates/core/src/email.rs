use std::str::FromStr;

use derive_more::{Display, Error};
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};

#[derive(Debug, Display, Error)]
#[display("invalid email address")]
pub struct ParseEmailError;

/// A syntactically valid email address in its canonical form: trimmed and lowercased, at
/// most 254 characters.
///
/// Normalizing at parse time makes the stored and compared form unique, so an account
/// cannot be looked up or created under two spellings of the same address.
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Email(String);

impl Email {
    const MAX_LEN: usize = 254;

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Email {
    type Err = ParseEmailError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let email = value.trim().to_lowercase();

        if email.len() > Self::MAX_LEN || !EmailAddress::is_valid(&email) {
            return Err(ParseEmailError);
        }

        Ok(Self(email))
    }
}

impl TryFrom<String> for Email {
    type Error = ParseEmailError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<Email> for String {
    fn from(email: Email) -> Self {
        email.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_and_surrounding_whitespace() {
        let email: Email = " Person@Example.test ".parse().unwrap();

        assert_eq!(email.as_str(), "person@example.test");
    }

    #[test]
    fn rejects_invalid_and_oversized_addresses() {
        let oversized = format!("{}@example.test", "a".repeat(250));

        for value in ["invalid", "", "a@", oversized.as_str()] {
            assert!(value.parse::<Email>().is_err(), "{value}");
        }
    }

    #[test]
    fn deserializes_through_the_same_validation() {
        assert!(serde_json::from_str::<Email>("\"Person@Example.test\"").is_ok());
        assert!(serde_json::from_str::<Email>("\"nope\"").is_err());
    }
}
