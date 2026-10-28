use strum::IntoStaticStr;
use thiserror::Error;

/// Rejection reasons whose `Display` output is safe to return to clients: it
/// never contains paths, file content, client-supplied strings or internals.
#[derive(Error, IntoStaticStr, Debug, Clone, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum UploadError {
    #[error("too many files in one upload (maximum {max})")]
    TooManyFiles { max: u32 },
    #[error("file exceeds the maximum size of {max_bytes} bytes")]
    FileTooLarge { max_bytes: u64 },
    #[error("file is empty")]
    EmptyFile,
    #[error("unsupported file type")]
    UnsupportedFormat,
    #[error("declared file type does not match the file content")]
    TypeMismatch,
    #[error("image header is malformed or truncated")]
    MalformedImage,
    #[error("image exceeds the maximum resolution of {max_megapixels} megapixels")]
    ImageTooLarge { max_megapixels: u32 },
}

impl UploadError {
    /// Stable machine-readable identifier for API error bodies.
    pub fn code(&self) -> &'static str {
        self.into()
    }
}
