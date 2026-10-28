const FALLBACK_NAME: &str = "upload";
const MAX_STEM_CHARS: usize = 100;
const MAX_EXTENSION_CHARS: usize = 16;

/// Client filename reduced to a harmless label.
///
/// It is for display only: storage keys must be generated server-side. Only
/// ASCII alphanumerics, `.`, `-` and `_` survive, so path separators, control
/// characters, NUL and bidirectional overrides can never reach a path or a log
/// line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientFilename {
    stem: String,
    extension: Option<String>,
}

impl ClientFilename {
    pub fn sanitize(raw: &str) -> Self {
        let basename = raw.rsplit(['/', '\\']).next().unwrap_or_default();
        let cleaned: String = basename
            .chars()
            .map(|c| if is_safe(c) { c } else { '_' })
            .collect();
        let cleaned = cleaned.trim_start_matches('.');

        let (stem, extension) = match cleaned.rsplit_once('.') {
            Some((stem, extension)) if !extension.is_empty() => (stem, Some(extension)),
            Some((stem, _)) => (stem, None),
            None => (cleaned, None),
        };
        let stem = stem.trim_end_matches('.');
        let stem = if stem.is_empty() { FALLBACK_NAME } else { stem };

        Self {
            stem: stem.chars().take(MAX_STEM_CHARS).collect(),
            extension: extension.map(|extension| {
                extension
                    .chars()
                    .take(MAX_EXTENSION_CHARS)
                    .collect::<String>()
                    .to_ascii_lowercase()
            }),
        }
    }

    /// Lowercase extension without the dot, if the client sent one.
    pub fn extension(&self) -> Option<&str> {
        self.extension.as_deref()
    }

    pub fn to_display_string(&self) -> String {
        match &self.extension {
            Some(extension) => format!("{}.{extension}", self.stem),
            None => self.stem.clone(),
        }
    }
}

fn is_safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sanitized(raw: &str) -> String {
        ClientFilename::sanitize(raw).to_display_string()
    }

    #[test]
    fn plain_name_is_kept_with_lowercase_extension() {
        let name = ClientFilename::sanitize("IMG_0001.JPG");

        assert_eq!(name.to_display_string(), "IMG_0001.jpg");
        assert_eq!(name.extension(), Some("jpg"));
    }

    #[test]
    fn path_components_are_dropped() {
        assert_eq!(sanitized("../../etc/passwd"), "passwd");
        assert_eq!(sanitized("C:\\Users\\me\\a.jpg"), "a.jpg");
        assert_eq!(sanitized("/abs/path/a.jpg"), "a.jpg");
    }

    #[test]
    fn traversal_only_names_fall_back() {
        assert_eq!(sanitized(".."), "upload");
        assert_eq!(sanitized("a/.."), "upload");
        assert_eq!(sanitized(""), "upload");
        assert_eq!(sanitized("/"), "upload");
    }

    #[test]
    fn unsafe_characters_are_replaced() {
        assert_eq!(sanitized("a\0b\n\u{202e}c d.jpg"), "a_b__c_d.jpg");
        assert_eq!(sanitized("é.jpg"), "_.jpg");
    }

    #[test]
    fn leading_dots_are_stripped() {
        assert_eq!(sanitized(".hidden.jpg"), "hidden.jpg");
    }

    #[test]
    fn only_the_last_extension_counts() {
        let name = ClientFilename::sanitize("photo.jpg.exe");

        assert_eq!(name.extension(), Some("exe"));
    }

    #[test]
    fn missing_extension_is_none() {
        assert_eq!(ClientFilename::sanitize("photo").extension(), None);
        assert_eq!(ClientFilename::sanitize("photo.").extension(), None);
    }

    #[test]
    fn long_names_are_truncated_but_keep_the_extension() {
        let raw = format!("{}.jpg", "a".repeat(500));

        let name = ClientFilename::sanitize(&raw);

        assert_eq!(
            name.to_display_string().len(),
            MAX_STEM_CHARS + ".jpg".len()
        );
        assert_eq!(name.extension(), Some("jpg"));
    }
}
