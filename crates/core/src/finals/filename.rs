use super::{error::FinalError, format::ImageFormat};

const MAX_FILENAME_CHARS: usize = 128;

/// A display-only name. It must never be used as a storage key or path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeFilename(String);

impl SafeFilename {
    /// Rejects traversal, separators, control characters and over-long names;
    /// other unusual characters are replaced. The extension must match the
    /// format detected from the content.
    pub fn parse(raw: &str, format: ImageFormat) -> Result<Self, FinalError> {
        let raw = raw.trim();

        if raw.is_empty()
            || raw.chars().count() > MAX_FILENAME_CHARS
            || raw.contains(['/', '\\'])
            || raw.chars().any(char::is_control)
            || raw.contains("..")
        {
            return Err(FinalError::InvalidFilename);
        }

        let sanitised: String = raw
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let sanitised = sanitised.trim_start_matches('.').to_owned();

        let (stem, extension) = sanitised
            .rsplit_once('.')
            .ok_or(FinalError::InvalidFilename)?;

        if stem.trim().is_empty() || !format.matches_extension(extension) {
            return Err(FinalError::InvalidFilename);
        }

        Ok(Self(sanitised))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Result<SafeFilename, FinalError> {
        SafeFilename::parse(raw, ImageFormat::Jpeg)
    }

    #[test]
    fn plain_name_is_kept() {
        assert_eq!(
            parse("Wedding 01.JPG").expect("valid").as_str(),
            "Wedding 01.JPG"
        );
    }

    #[test]
    fn unusual_characters_are_replaced() {
        assert_eq!(parse("a<b>:c.jpeg").expect("valid").as_str(), "a_b__c.jpeg");
    }

    #[test]
    fn bidi_override_is_replaced() {
        assert_eq!(parse("x\u{202e}.jpg").expect("valid").as_str(), "x_.jpg");
    }

    #[test]
    fn traversal_separators_and_control_characters_are_rejected() {
        for name in [
            "../x.jpg",
            "a/b.jpg",
            "a\\b.jpg",
            "..\\x.jpg",
            "a..b.jpg",
            "x.jpg\0",
            "x\n.jpg",
            "..",
            ".",
            "",
            "   ",
        ] {
            assert_eq!(parse(name), Err(FinalError::InvalidFilename), "{name:?}");
        }
    }

    #[test]
    fn missing_empty_stem_or_wrong_extension_is_rejected() {
        for name in ["noext", ".jpg", "x.png", "x.jpg.exe", "x."] {
            assert_eq!(parse(name), Err(FinalError::InvalidFilename), "{name:?}");
        }
    }

    #[test]
    fn name_length_is_bounded() {
        let ok = format!("{}.jpg", "a".repeat(MAX_FILENAME_CHARS - 4));
        let long = format!("{}.jpg", "a".repeat(MAX_FILENAME_CHARS - 3));

        assert!(parse(&ok).is_ok());
        assert_eq!(parse(&long), Err(FinalError::InvalidFilename));
    }
}
