use std::str::FromStr;

use derive_more::Display;
use email_address::EmailAddress;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::bounded::{BoundedText, MultiLine, SingleLine};

/// Maximum length in Unicode scalar values of an [`EmailSubject`].
pub const MAX_EMAIL_SUBJECT_CHARS: usize = 200;

/// Maximum length in Unicode scalar values of an [`EmailMessage`].
pub const MAX_EMAIL_MESSAGE_CHARS: usize = 5000;

#[derive(Debug, Error)]
#[error("invalid email address")]
pub struct ParseEmailError;

/// A syntactically valid email address in its canonical form: trimmed and lowercased, at
/// most 254 characters.
///
/// Normalizing at parse time makes the stored and compared form unique, so an account
/// cannot be looked up or created under two spellings of the same address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Display, Serialize, Deserialize)]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type), sqlx(transparent))]
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

/// Validated single-line email subject (`1..=200` trimmed characters, rejecting `\r`, `\n`, `\0`
/// so header injection is unrepresentable).
pub type EmailSubject = BoundedText<1, MAX_EMAIL_SUBJECT_CHARS, SingleLine>;

/// Validated plain-text personal message body (`1..=5000` trimmed characters, preserving
/// paragraph line breaks while rejecting `\0`).
pub type EmailMessage = BoundedText<1, MAX_EMAIL_MESSAGE_CHARS, MultiLine>;

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

    #[test]
    fn email_subject_trims_and_rejects_empty_oversized_or_header_injection() {
        let subject: EmailSubject = "  Camille & Hugo — Your gallery is ready  "
            .parse()
            .unwrap();

        assert_eq!(subject.as_str(), "Camille & Hugo — Your gallery is ready");

        let max_subject = "é".repeat(200);

        assert_eq!(
            max_subject.parse::<EmailSubject>().unwrap().as_str(),
            max_subject
        );

        for invalid in [
            "",
            "   ",
            "Hello\r\nBcc: attacker@example.test",
            "Hello\nWorld",
            "Hello\rWorld",
            "Hello\0World",
            &"a".repeat(201),
        ] {
            assert!(invalid.parse::<EmailSubject>().is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn email_message_preserves_newlines_and_rejects_empty_oversized_or_nul() {
        let message: EmailMessage = "  Bonjour Camille,\n\nVoici vos photos.  ".parse().unwrap();

        assert_eq!(message.as_str(), "Bonjour Camille,\n\nVoici vos photos.");

        let max_message = "é".repeat(5000);

        assert_eq!(
            max_message.parse::<EmailMessage>().unwrap().as_str(),
            max_message
        );

        for invalid in ["", "   \n\t  ", "Hello\0World", &"a".repeat(5001)] {
            assert!(invalid.parse::<EmailMessage>().is_err(), "{invalid:?}");
        }
    }
}
