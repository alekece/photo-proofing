mod checksum;
mod error;
mod filename;
mod format;

use uuid::Uuid;

pub use self::{
    checksum::Sha256Checksum,
    error::FinalError,
    filename::SafeFilename,
    format::{Dimensions, ImageFormat, Orientation},
};
use crate::config::finals::FinalsConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GalleryId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhotoId(pub Uuid);

/// What the validator needs to know about the photo a final is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePhotoInfo {
    pub gallery_id: GalleryId,
    pub photo_id: PhotoId,
    /// Display dimensions (EXIF orientation applied). Finals are compared by
    /// their stored header dimensions: their EXIF orientation is not applied
    /// yet, so a final must be pixel-oriented like its source.
    pub dimensions: Dimensions,
    pub proof_long_edge: u32,
}

/// An untrusted upload; every field is client-controlled.
#[derive(Debug, Clone, Copy)]
pub struct FinalCandidate<'a> {
    /// Gallery the photographer is adding the final to.
    pub gallery_id: GalleryId,
    pub filename: &'a str,
    pub declared_mime: Option<&'a str>,
    pub expected_checksum: Option<&'a str>,
    pub bytes: &'a [u8],
}

/// Validated metadata of a final, ready to be persisted by a later layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalRendition {
    pub gallery_id: GalleryId,
    pub photo_id: PhotoId,
    /// Display-only; storage keys must be generated, never derived from it.
    pub filename: SafeFilename,
    pub format: ImageFormat,
    pub dimensions: Dimensions,
    pub size_bytes: u64,
    pub checksum: Sha256Checksum,
}

/// Validates `candidate` against its `source` photo without decoding pixels.
///
/// The gallery association is checked first so that nothing about a photo of
/// another gallery can leak through other errors.
pub fn validate_final(
    config: &FinalsConfig,
    source: &SourcePhotoInfo,
    candidate: &FinalCandidate<'_>,
) -> Result<FinalRendition, FinalError> {
    if candidate.gallery_id != source.gallery_id {
        return Err(FinalError::SourcePhotoNotFound);
    }

    let size_bytes = u64::try_from(candidate.bytes.len()).map_err(|_| FinalError::FileTooLarge)?;
    if size_bytes > config.finals_max_bytes {
        return Err(FinalError::FileTooLarge);
    }

    let expected_checksum = candidate
        .expected_checksum
        .map(Sha256Checksum::parse)
        .transpose()?;

    let format = ImageFormat::detect(candidate.bytes)?;
    if candidate
        .declared_mime
        .is_some_and(|mime| !format.matches_mime(mime))
    {
        return Err(FinalError::FormatMismatch);
    }

    let dimensions = format.read_dimensions(candidate.bytes)?;
    if dimensions.pixels() > config.max_pixels() {
        return Err(FinalError::TooManyPixels);
    }
    if dimensions.long_edge() < source.proof_long_edge {
        return Err(FinalError::SmallerThanProof);
    }
    if dimensions.orientation() != source.dimensions.orientation() {
        return Err(FinalError::OrientationMismatch);
    }

    let filename = SafeFilename::parse(candidate.filename, format)?;

    let checksum = Sha256Checksum::of(candidate.bytes);
    if expected_checksum.is_some_and(|expected| expected != checksum) {
        return Err(FinalError::ChecksumMismatch);
    }

    Ok(FinalRendition {
        gallery_id: source.gallery_id,
        photo_id: source.photo_id,
        filename,
        format,
        dimensions,
        size_bytes,
        checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::{format::fixtures, *};

    const GALLERY: GalleryId = GalleryId(Uuid::from_u128(1));

    fn source() -> SourcePhotoInfo {
        SourcePhotoInfo {
            gallery_id: GALLERY,
            photo_id: PhotoId(Uuid::from_u128(2)),
            dimensions: Dimensions {
                width: 6000,
                height: 4000,
            },
            proof_long_edge: 2000,
        }
    }

    fn candidate(bytes: &[u8]) -> FinalCandidate<'_> {
        FinalCandidate {
            gallery_id: GALLERY,
            filename: "final.png",
            declared_mime: Some("image/png"),
            expected_checksum: None,
            bytes,
        }
    }

    fn validate(
        config: &FinalsConfig,
        candidate: &FinalCandidate<'_>,
    ) -> Result<FinalRendition, FinalError> {
        validate_final(config, &source(), candidate)
    }

    #[test]
    fn valid_final_carries_metadata_and_checksum() {
        let bytes = fixtures::png(3000, 2000);
        let checksum = Sha256Checksum::of(&bytes).to_string();
        let candidate = FinalCandidate {
            expected_checksum: Some(&checksum),
            ..candidate(&bytes)
        };

        let rendition = validate(&FinalsConfig::default(), &candidate).expect("valid final");

        assert_eq!(rendition.gallery_id, GALLERY);
        assert_eq!(rendition.photo_id, source().photo_id);
        assert_eq!(rendition.format, ImageFormat::Png);
        assert_eq!(
            rendition.dimensions,
            Dimensions {
                width: 3000,
                height: 2000
            }
        );
        assert_eq!(rendition.size_bytes, bytes.len() as u64);
        assert_eq!(rendition.checksum.to_string(), checksum);
    }

    #[test]
    fn size_limit_is_inclusive() {
        let bytes = fixtures::png(3000, 2000);
        let size = bytes.len() as u64;
        let at_limit = FinalsConfig {
            finals_max_bytes: size,
            ..FinalsConfig::default()
        };
        let below = FinalsConfig {
            finals_max_bytes: size - 1,
            ..FinalsConfig::default()
        };

        assert!(validate(&at_limit, &candidate(&bytes)).is_ok());
        assert_eq!(
            validate(&below, &candidate(&bytes)),
            Err(FinalError::FileTooLarge)
        );
    }

    #[test]
    fn pixel_limit_is_inclusive() {
        let config = FinalsConfig {
            finals_max_megapixels: 6,
            ..FinalsConfig::default()
        };
        let at_limit = fixtures::png(3000, 2000);
        let over = fixtures::png(3001, 2000);

        assert!(validate(&config, &candidate(&at_limit)).is_ok());
        assert_eq!(
            validate(&config, &candidate(&over)),
            Err(FinalError::TooManyPixels)
        );
    }

    #[test]
    fn huge_dimensions_do_not_overflow() {
        let bytes = fixtures::png(u32::MAX, u32::MAX);

        assert_eq!(
            validate(&FinalsConfig::default(), &candidate(&bytes)),
            Err(FinalError::TooManyPixels)
        );
    }

    #[test]
    fn long_edge_must_reach_proof_long_edge() {
        let at_proof = fixtures::png(2000, 1333);
        let below = fixtures::png(1999, 1333);

        assert!(validate(&FinalsConfig::default(), &candidate(&at_proof)).is_ok());
        assert_eq!(
            validate(&FinalsConfig::default(), &candidate(&below)),
            Err(FinalError::SmallerThanProof)
        );
    }

    #[test]
    fn orientation_must_match_source() {
        let portrait = fixtures::png(2000, 3000);
        let square = fixtures::png(3000, 3000);

        for bytes in [&portrait, &square] {
            assert_eq!(
                validate(&FinalsConfig::default(), &candidate(bytes)),
                Err(FinalError::OrientationMismatch)
            );
        }
    }

    #[test]
    fn jpeg_and_tiff_are_accepted() {
        let jpeg = fixtures::jpeg(3000, 2000);
        let tiff = fixtures::tiff(3000, 2000);
        let jpeg_candidate = FinalCandidate {
            filename: "a.jpg",
            declared_mime: Some("image/jpeg"),
            ..candidate(&jpeg)
        };
        let tiff_candidate = FinalCandidate {
            filename: "a.tif",
            declared_mime: None,
            ..candidate(&tiff)
        };

        assert!(validate(&FinalsConfig::default(), &jpeg_candidate).is_ok());
        assert!(validate(&FinalsConfig::default(), &tiff_candidate).is_ok());
    }

    #[test]
    fn spoofed_type_is_rejected() {
        let png = fixtures::png(3000, 2000);
        let mut gif = b"GIF89a".to_vec();
        gif.extend(&png[6..]);
        let jpeg_named = FinalCandidate {
            filename: "a.jpg",
            ..candidate(&png)
        };
        let jpeg_mime = FinalCandidate {
            declared_mime: Some("image/jpeg"),
            ..candidate(&png)
        };

        assert_eq!(
            validate(&FinalsConfig::default(), &candidate(&gif)),
            Err(FinalError::UnsupportedFormat)
        );
        assert_eq!(
            validate(&FinalsConfig::default(), &jpeg_named),
            Err(FinalError::InvalidFilename)
        );
        assert_eq!(
            validate(&FinalsConfig::default(), &jpeg_mime),
            Err(FinalError::FormatMismatch)
        );
    }

    #[test]
    fn truncated_header_is_rejected() {
        let bytes = fixtures::png(3000, 2000);

        assert_eq!(
            validate(&FinalsConfig::default(), &candidate(&bytes[..16])),
            Err(FinalError::InvalidHeader)
        );
    }

    #[test]
    fn bad_filename_is_rejected() {
        let bytes = fixtures::png(3000, 2000);
        let candidate = FinalCandidate {
            filename: "../../etc/passwd.png",
            ..candidate(&bytes)
        };

        assert_eq!(
            validate(&FinalsConfig::default(), &candidate),
            Err(FinalError::InvalidFilename)
        );
    }

    #[test]
    fn checksum_mismatch_and_malformed_checksum_are_rejected() {
        let bytes = fixtures::png(3000, 2000);
        let other = Sha256Checksum::of(b"other").to_string();
        let mismatch = FinalCandidate {
            expected_checksum: Some(&other),
            ..candidate(&bytes)
        };
        let malformed = FinalCandidate {
            expected_checksum: Some("zz"),
            ..candidate(&bytes)
        };

        assert_eq!(
            validate(&FinalsConfig::default(), &mismatch),
            Err(FinalError::ChecksumMismatch)
        );
        assert_eq!(
            validate(&FinalsConfig::default(), &malformed),
            Err(FinalError::InvalidChecksum)
        );
    }

    #[test]
    fn cross_gallery_association_fails_opaquely_before_inspecting_content() {
        let bytes = fixtures::png(10, 10);
        let candidate = FinalCandidate {
            gallery_id: GalleryId(Uuid::from_u128(99)),
            ..candidate(&bytes)
        };

        let error = validate(&FinalsConfig::default(), &candidate).expect_err("cross-gallery");

        assert_eq!(error, FinalError::SourcePhotoNotFound);
        assert_eq!(error.to_string(), "source photo not found");
    }
}
