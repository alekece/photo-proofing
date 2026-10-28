use std::fmt::{self, Display, Formatter};

use sha2::{Digest, Sha256};

use super::error::FinalError;

const DIGEST_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sha256Checksum([u8; DIGEST_LEN]);

impl Sha256Checksum {
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Parses a 64-character hexadecimal digest, case-insensitively.
    pub fn parse(hex: &str) -> Result<Self, FinalError> {
        let hex = hex.trim().as_bytes();

        if hex.len() != DIGEST_LEN * 2 {
            return Err(FinalError::InvalidChecksum);
        }

        let mut digest = [0; DIGEST_LEN];
        let (pairs, _) = hex.as_chunks::<2>();
        for (byte, [high, low]) in digest.iter_mut().zip(pairs) {
            let high = hex_value(*high)?;
            let low = hex_value(*low)?;
            *byte = high << 4 | low;
        }

        Ok(Self(digest))
    }
}

impl Display for Sha256Checksum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

fn hex_value(digit: u8) -> Result<u8, FinalError> {
    char::from(digit)
        .to_digit(16)
        .and_then(|value| u8::try_from(value).ok())
        .ok_or(FinalError::InvalidChecksum)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn digest_matches_known_vector_and_round_trips() {
        let checksum = Sha256Checksum::of(b"");

        assert_eq!(checksum.to_string(), EMPTY_SHA256);
        assert_eq!(
            Sha256Checksum::parse(&EMPTY_SHA256.to_uppercase()),
            Ok(checksum)
        );
    }

    #[test]
    fn malformed_digest_is_rejected() {
        for hex in ["", "abc", &"g".repeat(64), &"a".repeat(63), &"é".repeat(32)] {
            assert_eq!(
                Sha256Checksum::parse(hex),
                Err(FinalError::InvalidChecksum),
                "{hex:?}"
            );
        }
    }
}
