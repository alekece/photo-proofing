//! Framework-independent validation of uploaded originals.
//!
//! A transport adapter creates one [`UploadSession`] per request, calls
//! [`UploadSession::start_file`] for every part and feeds the part's chunks to
//! the returned [`FileValidator`]. Every limit is enforced as data arrives, so
//! an oversized or malformed upload is refused without buffering it: at most
//! [`HEADER_WINDOW`] bytes per file are ever held.
//!
//! What is checked, per file:
//! - size, from the byte count seen so far (exceeding the limit aborts at once);
//! - format, from the file signature; a declared MIME type or extension that
//!   disagrees with the signature is rejected;
//! - JPEG resolution, from the frame header without decoding;
//! - filename, reduced to a harmless label (never use it as a storage key).
//!
//! What is not checked: RAW resolution. Canon, Nikon, Sony and Adobe RAW keep
//! the full-size sensor dimensions in vendor-specific sub-IFDs or boxes, and
//! IFD0 usually describes an embedded preview, so a header-only value would be
//! misleading. RAW files are bounded by the byte limit and must be bounded
//! again by whatever later decodes them. Likewise only the container header is
//! verified, not the integrity of the remaining bytes.

pub mod error;
pub mod filename;
pub mod format;
pub mod jpeg;

#[cfg(test)]
mod fixtures;

use crate::{
    config::upload::UploadConfig,
    upload::{
        error::UploadError, filename::ClientFilename, format::UploadFormat, jpeg::Dimensions,
    },
};

/// Leading bytes buffered per file for signature and header inspection. JPEG
/// files with unusually large metadata before the frame header, and TIFF RAW
/// files whose Make tag lies further in, are rejected as malformed.
pub const HEADER_WINDOW: usize = 256 * 1024;

const PIXELS_PER_MEGAPIXEL: u64 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadLimits {
    pub max_files: u32,
    pub max_file_bytes: u64,
    pub max_megapixels: u32,
}

impl UploadLimits {
    fn max_pixels(self) -> u64 {
        u64::from(self.max_megapixels) * PIXELS_PER_MEGAPIXEL
    }
}

impl From<&UploadConfig> for UploadLimits {
    fn from(config: &UploadConfig) -> Self {
        Self {
            max_files: config.upload_max_files,
            max_file_bytes: config.upload_max_file_bytes,
            max_megapixels: config.upload_max_megapixels,
        }
    }
}

/// Outcome for a file that passed validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFile {
    pub format: UploadFormat,
    pub size_bytes: u64,
    /// Known for JPEG only; see the module documentation.
    pub dimensions: Option<Dimensions>,
    pub filename: ClientFilename,
}

/// Per-request state enforcing the file-count limit.
#[derive(Debug)]
pub struct UploadSession {
    limits: UploadLimits,
    files_started: u32,
}

impl UploadSession {
    pub fn new(limits: UploadLimits) -> Self {
        Self {
            limits,
            files_started: 0,
        }
    }

    /// Registers the next file of the request. Fails as soon as the file
    /// beyond the limit is announced, before any of its bytes are read.
    pub fn start_file(
        &mut self,
        filename: Option<&str>,
        content_type: Option<&str>,
    ) -> Result<FileValidator, UploadError> {
        if self.files_started >= self.limits.max_files {
            return Err(UploadError::TooManyFiles {
                max: self.limits.max_files,
            });
        }
        self.files_started += 1;

        Ok(FileValidator {
            limits: self.limits,
            filename: ClientFilename::sanitize(filename.unwrap_or_default()),
            content_type: content_type.map(str::to_owned),
            size_bytes: 0,
            header: Vec::new(),
            inspected: None,
            rejection: None,
        })
    }
}

#[derive(Debug)]
struct Inspection {
    format: UploadFormat,
    dimensions: Option<Dimensions>,
}

/// Validates one file from its streamed chunks. After the first error every
/// further call returns that same error, so a caller that ignores a failure
/// cannot get a file accepted.
#[derive(Debug)]
pub struct FileValidator {
    limits: UploadLimits,
    filename: ClientFilename,
    content_type: Option<String>,
    size_bytes: u64,
    header: Vec<u8>,
    inspected: Option<Inspection>,
    rejection: Option<UploadError>,
}

impl FileValidator {
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), UploadError> {
        self.check_rejection()?;

        let result = self.accept(chunk);

        self.record(result)
    }

    pub fn finish(mut self) -> Result<ValidatedFile, UploadError> {
        self.check_rejection()?;

        let result = self.complete();

        self.record(result)?;
        let inspection = self.inspected.take().ok_or(UploadError::MalformedImage)?;

        Ok(ValidatedFile {
            format: inspection.format,
            size_bytes: self.size_bytes,
            dimensions: inspection.dimensions,
            filename: self.filename,
        })
    }

    fn accept(&mut self, chunk: &[u8]) -> Result<(), UploadError> {
        let chunk_len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
        self.size_bytes = self.size_bytes.saturating_add(chunk_len);
        if self.size_bytes > self.limits.max_file_bytes {
            return Err(UploadError::FileTooLarge {
                max_bytes: self.limits.max_file_bytes,
            });
        }
        if self.inspected.is_some() {
            return Ok(());
        }

        let room = HEADER_WINDOW - self.header.len();
        self.header
            .extend_from_slice(&chunk[..chunk.len().min(room)]);
        if self.header.len() == HEADER_WINDOW {
            self.inspect_header()?;
        }

        Ok(())
    }

    fn complete(&mut self) -> Result<(), UploadError> {
        if self.size_bytes == 0 {
            return Err(UploadError::EmptyFile);
        }
        if self.inspected.is_none() {
            self.inspect_header()?;
        }

        Ok(())
    }

    fn inspect_header(&mut self) -> Result<(), UploadError> {
        let format = UploadFormat::detect(&self.header).ok_or(UploadError::UnsupportedFormat)?;

        let content_type_matches = self
            .content_type
            .as_deref()
            .is_none_or(|declared| format.accepts_content_type(declared));
        let extension_matches = self
            .filename
            .extension()
            .is_none_or(|extension| format.accepts_extension(extension));
        if !content_type_matches || !extension_matches {
            return Err(UploadError::TypeMismatch);
        }

        let dimensions = match format {
            UploadFormat::Jpeg => Some(self.check_jpeg_dimensions()?),
            _ => None,
        };

        self.inspected = Some(Inspection { format, dimensions });
        self.header = Vec::new();

        Ok(())
    }

    fn check_jpeg_dimensions(&self) -> Result<Dimensions, UploadError> {
        let dimensions = jpeg::read_dimensions(&self.header)?;
        if dimensions.pixels() > self.limits.max_pixels() {
            return Err(UploadError::ImageTooLarge {
                max_megapixels: self.limits.max_megapixels,
            });
        }

        Ok(dimensions)
    }

    fn check_rejection(&self) -> Result<(), UploadError> {
        self.rejection.clone().map_or(Ok(()), Err)
    }

    fn record(&mut self, result: Result<(), UploadError>) -> Result<(), UploadError> {
        if let Err(error) = &result {
            self.rejection = Some(error.clone());
            self.header = Vec::new();
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    const LIMITS: UploadLimits = UploadLimits {
        max_files: 3,
        max_file_bytes: 1_000,
        max_megapixels: 1,
    };

    fn session() -> UploadSession {
        UploadSession::new(LIMITS)
    }

    fn validate(
        limits: UploadLimits,
        filename: Option<&str>,
        content_type: Option<&str>,
        chunks: &[&[u8]],
    ) -> Result<ValidatedFile, UploadError> {
        let mut validator = UploadSession::new(limits).start_file(filename, content_type)?;
        for chunk in chunks {
            validator.push(chunk)?;
        }

        validator.finish()
    }

    fn padded(mut bytes: Vec<u8>, total: usize) -> Vec<u8> {
        bytes.resize(total, 0);
        bytes
    }

    #[test]
    fn file_count_limit_passes_at_max_and_fails_one_over() {
        let mut session = session();

        for _ in 0..LIMITS.max_files {
            session.start_file(None, None).expect("within the limit");
        }

        assert_eq!(
            session.start_file(None, None).err(),
            Some(UploadError::TooManyFiles { max: 3 })
        );
    }

    #[test]
    fn size_limit_passes_at_max_and_fails_one_over() {
        let at_limit = padded(fixtures::jpeg(10, 10), 1_000);
        let over_limit = padded(fixtures::jpeg(10, 10), 1_001);

        let accepted = validate(LIMITS, None, None, &[&at_limit]).expect("exact limit passes");

        assert_eq!(accepted.size_bytes, 1_000);
        assert_eq!(
            validate(LIMITS, None, None, &[&over_limit]),
            Err(UploadError::FileTooLarge { max_bytes: 1_000 })
        );
    }

    #[test]
    fn oversize_is_rejected_on_the_chunk_that_crosses_the_limit() {
        let mut validator = session().start_file(None, None).expect("start");
        let jpeg = padded(fixtures::jpeg(10, 10), 600);

        validator.push(&jpeg).expect("first chunk fits");
        let error = validator
            .push(&jpeg)
            .expect_err("second chunk crosses the limit");

        assert_eq!(error, UploadError::FileTooLarge { max_bytes: 1_000 });
        assert_eq!(validator.push(&[0]), Err(error.clone()), "stays rejected");
        assert_eq!(validator.finish().err(), Some(error));
    }

    #[test]
    fn size_limit_counts_bytes_beyond_the_header_window() {
        let limits = UploadLimits {
            max_file_bytes: (HEADER_WINDOW as u64) + 10,
            ..LIMITS
        };
        let mut validator = UploadSession::new(limits)
            .start_file(None, None)
            .expect("start");
        validator
            .push(&padded(fixtures::jpeg(10, 10), HEADER_WINDOW))
            .expect("window fits");

        validator.push(&[0; 10]).expect("exact limit passes");
        let error = validator.push(&[0]).expect_err("one byte over");

        assert_eq!(
            error,
            UploadError::FileTooLarge {
                max_bytes: limits.max_file_bytes
            }
        );
    }

    #[test]
    fn header_buffer_never_exceeds_the_window() {
        let mut validator = UploadSession::new(UploadLimits {
            max_file_bytes: u64::MAX,
            ..LIMITS
        })
        .start_file(None, None)
        .expect("start");

        validator
            .push(&padded(fixtures::jpeg(10, 10), HEADER_WINDOW * 3))
            .expect("push");

        assert!(validator.header.capacity() <= HEADER_WINDOW * 2);
        assert!(validator.header.is_empty(), "released once inspected");
    }

    #[test]
    fn pixel_limit_passes_at_max_and_fails_one_over() {
        let at_limit = fixtures::jpeg(1_000, 1_000);
        let over_limit = fixtures::jpeg(1_000, 1_001);

        let accepted = validate(LIMITS, None, None, &[&at_limit]).expect("exact limit passes");

        assert_eq!(
            accepted.dimensions,
            Some(Dimensions {
                width: 1_000,
                height: 1_000
            })
        );
        assert_eq!(
            validate(LIMITS, None, None, &[&over_limit]),
            Err(UploadError::ImageTooLarge { max_megapixels: 1 })
        );
    }

    #[test]
    fn default_limit_accepts_100_megapixels_and_rejects_more() {
        let limits = UploadLimits {
            max_files: 1,
            max_file_bytes: 1 << 20,
            max_megapixels: 100,
        };

        assert!(validate(limits, None, None, &[&fixtures::jpeg(10_000, 10_000)]).is_ok());
        assert_eq!(
            validate(limits, None, None, &[&fixtures::jpeg(10_001, 10_000)]),
            Err(UploadError::ImageTooLarge {
                max_megapixels: 100
            })
        );
    }

    #[test]
    fn every_format_is_accepted_by_signature() {
        let cases = [
            (UploadFormat::Jpeg, fixtures::jpeg(10, 10)),
            (UploadFormat::Cr2, fixtures::cr2()),
            (UploadFormat::Cr3, fixtures::cr3()),
            (UploadFormat::Nef, fixtures::nef()),
            (UploadFormat::Arw, fixtures::arw()),
            (UploadFormat::Dng, fixtures::dng()),
            (UploadFormat::Raf, fixtures::raf()),
        ];
        assert_eq!(cases.len(), UploadFormat::iter().count());

        for (format, bytes) in cases {
            let name = format!(
                "IMG.{}",
                if format == UploadFormat::Jpeg {
                    "JPG".into()
                } else {
                    format.to_string()
                }
            );

            let accepted = validate(LIMITS, Some(&name), None, &[&bytes]).expect("accepted");

            assert_eq!(accepted.format, format);
            assert_eq!(accepted.dimensions.is_some(), !format.is_raw());
        }
    }

    #[test]
    fn chunk_boundaries_do_not_matter() {
        let bytes = fixtures::jpeg(10, 10);
        let chunks: Vec<&[u8]> = bytes.chunks(1).collect();

        let accepted = validate(LIMITS, None, None, &chunks).expect("byte-wise stream");

        assert_eq!(accepted.format, UploadFormat::Jpeg);
    }

    #[test]
    fn zero_byte_file_is_rejected() {
        assert_eq!(
            validate(LIMITS, None, None, &[]),
            Err(UploadError::EmptyFile)
        );
        assert_eq!(
            validate(LIMITS, None, None, &[&[]]),
            Err(UploadError::EmptyFile)
        );
    }

    #[test]
    fn spoofed_signature_is_rejected_whatever_the_client_declares() {
        let html = b"<html><script>alert(1)</script></html>";

        let error = validate(LIMITS, Some("a.jpg"), Some("image/jpeg"), &[html]);

        assert_eq!(error, Err(UploadError::UnsupportedFormat));
    }

    #[test]
    fn declared_type_must_match_the_signature() {
        let jpeg = fixtures::jpeg(10, 10);
        let nef = fixtures::nef();

        assert_eq!(
            validate(LIMITS, None, Some("image/png"), &[&jpeg]),
            Err(UploadError::TypeMismatch)
        );
        assert_eq!(
            validate(LIMITS, Some("a.png"), None, &[&jpeg]),
            Err(UploadError::TypeMismatch)
        );
        assert_eq!(
            validate(LIMITS, Some("a.jpg"), None, &[&nef]),
            Err(UploadError::TypeMismatch)
        );
        assert_eq!(
            validate(LIMITS, Some("a.nef"), Some("image/jpeg"), &[&nef]),
            Err(UploadError::TypeMismatch)
        );
        assert_eq!(
            validate(LIMITS, Some("photo.jpg.exe"), None, &[&jpeg]),
            Err(UploadError::TypeMismatch)
        );
    }

    #[test]
    fn truncated_and_malformed_headers_are_rejected() {
        let jpeg = fixtures::jpeg(10, 10);
        let truncated = &jpeg[..jpeg.len() - 6];

        assert_eq!(
            validate(LIMITS, None, None, &[truncated]),
            Err(UploadError::MalformedImage)
        );
        assert_eq!(
            validate(LIMITS, None, None, &[&[0xFF, 0xD8, 0xFF]]),
            Err(UploadError::MalformedImage)
        );
        assert_eq!(
            validate(LIMITS, None, None, &[&[0xFF]]),
            Err(UploadError::UnsupportedFormat)
        );
        assert_eq!(
            validate(LIMITS, None, None, &[&fixtures::raf()[..8]]),
            Err(UploadError::UnsupportedFormat)
        );
    }

    #[test]
    fn jpeg_whose_frame_header_lies_beyond_the_window_is_rejected() {
        let mut bytes = vec![0xFF, 0xD8];
        for _ in 0..5 {
            bytes.extend([0xFF, 0xE1, 0xFF, 0xFF]);
            bytes.resize(bytes.len() + 0xFFFD, 0);
        }
        bytes.extend(&fixtures::jpeg(10, 10)[2..]);
        let limits = UploadLimits {
            max_file_bytes: u64::MAX,
            ..LIMITS
        };

        assert_eq!(
            validate(limits, None, None, &[&bytes]),
            Err(UploadError::MalformedImage)
        );
    }

    #[test]
    fn client_filename_is_sanitised() {
        let jpeg = fixtures::jpeg(10, 10);

        let accepted =
            validate(LIMITS, Some("../../etc/passwd.JPG"), None, &[&jpeg]).expect("accepted");

        assert_eq!(accepted.filename.to_display_string(), "passwd.jpg");
    }

    #[test]
    fn error_messages_do_not_echo_client_input() {
        let secret = "../../secret-name\u{202e}";
        let error = validate(
            LIMITS,
            Some(secret),
            Some("text/secret"),
            &[b"secret-content"],
        )
        .expect_err("rejected");

        let message = error.to_string();

        assert!(!message.contains("secret"));
        assert_eq!(error.code(), "unsupported_format");
    }
}
