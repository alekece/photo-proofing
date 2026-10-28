use thiserror::Error;

/// Every message is safe to show to a client: none echoes user input, paths
/// or internal identifiers.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum FinalError {
    /// Also returned for a source photo in another gallery so that callers
    /// cannot probe which photos exist elsewhere.
    #[error("source photo not found")]
    SourcePhotoNotFound,
    #[error("unsupported or unrecognised image format")]
    UnsupportedFormat,
    #[error("declared type does not match the file content")]
    FormatMismatch,
    #[error("image header is truncated or malformed")]
    InvalidHeader,
    #[error("file is too large")]
    FileTooLarge,
    #[error("image has too many pixels")]
    TooManyPixels,
    #[error("final is smaller than the proof of its source photo")]
    SmallerThanProof,
    #[error("final orientation differs from its source photo")]
    OrientationMismatch,
    #[error("invalid filename")]
    InvalidFilename,
    #[error("invalid checksum format")]
    InvalidChecksum,
    #[error("checksum does not match the file content")]
    ChecksumMismatch,
}
