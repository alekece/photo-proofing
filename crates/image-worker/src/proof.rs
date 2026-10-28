use std::io::Cursor;

use eg_core::config::proof::ProofConfig;
use image::{
    DynamicImage, ExtendedColorType, ImageDecoder, ImageError, Limits, RgbImage,
    codecs::jpeg::{JpegDecoder, JpegEncoder},
    imageops::FilterType,
};
use moxcms::{ColorProfile, Layout, TransformOptions};
use thiserror::Error;

const JPEG_SIGNATURE: [u8; 3] = [0xFF, 0xD8, 0xFF];

/// Largest edge the JPEG format can express.
const JPEG_MAX_EDGE: u32 = 65_535;

#[derive(Error, Debug)]
pub enum ProofError {
    #[error("source is not a JPEG")]
    NotJpeg,
    #[error("source is {size} bytes, above the {limit} byte limit")]
    SourceTooLarge { size: u64, limit: u64 },
    #[error("source is {width}x{height} pixels, above the {limit} pixel limit")]
    DimensionsTooLarge { width: u32, height: u32, limit: u64 },
    #[error("source JPEG is corrupt or unsupported")]
    Decode(#[source] ImageError),
    #[error("proof encoding failed")]
    Encode(#[source] ImageError),
}

/// A generated proof: JPEG bytes carrying no source metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proof {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Pure, deterministic proof generation: the same source and configuration
/// always yield the same bytes, so a failed job can be retried safely.
///
/// The output is re-encoded from decoded pixels after the EXIF orientation is
/// applied. Nothing else is copied from the source, so EXIF (including GPS),
/// XMP, comments and embedded ICC profiles are all dropped. A source ICC
/// profile is first used to convert pixels to sRGB, which viewers assume for
/// profile-less JPEGs, so wide-gamut photos keep their look. Source filenames
/// are never read.
///
/// Memory is bounded by `proof_max_source_pixels` (the decoder's own limits
/// do not cap allocations): budget roughly 10 bytes per source pixel per job.
#[derive(Debug, Clone)]
pub struct ProofGenerator {
    config: ProofConfig,
}

impl ProofGenerator {
    pub fn new(config: ProofConfig) -> Self {
        Self { config }
    }

    pub fn generate(&self, source: &[u8]) -> Result<Proof, ProofError> {
        let size = source.len() as u64;
        if size > self.config.proof_max_source_bytes {
            return Err(ProofError::SourceTooLarge {
                size,
                limit: self.config.proof_max_source_bytes,
            });
        }
        if !source.starts_with(&JPEG_SIGNATURE) {
            return Err(ProofError::NotJpeg);
        }

        let mut decoder = JpegDecoder::new(Cursor::new(source)).map_err(ProofError::Decode)?;

        // Header dimensions are checked before any pixel buffer is allocated.
        let (width, height) = decoder.dimensions();
        let pixels = u64::from(width) * u64::from(height);
        if pixels > self.config.proof_max_source_pixels {
            return Err(ProofError::DimensionsTooLarge {
                width,
                height,
                limit: self.config.proof_max_source_pixels,
            });
        }

        let mut limits = Limits::default();
        limits.max_image_width = Some(JPEG_MAX_EDGE);
        limits.max_image_height = Some(JPEG_MAX_EDGE);
        decoder.set_limits(limits).map_err(ProofError::Decode)?;

        // Orientation must be read before the decoder is consumed and applied
        // before the metadata that carries it is discarded.
        let orientation = decoder.orientation().map_err(ProofError::Decode)?;
        let icc_profile = decoder.icc_profile().map_err(ProofError::Decode)?;
        let mut image = DynamicImage::from_decoder(decoder).map_err(ProofError::Decode)?;
        image.apply_orientation(orientation);

        let max_edge = self.config.proof_max_edge;
        if image.width().max(image.height()) > max_edge {
            image = image.resize(max_edge, max_edge, FilterType::Lanczos3);
        }

        let mut rgb = image.into_rgb8();
        if let Some(icc_profile) = icc_profile {
            convert_to_srgb(&mut rgb, &icc_profile);
        }

        let (width, height) = rgb.dimensions();
        let mut bytes = Vec::new();
        JpegEncoder::new_with_quality(&mut bytes, self.config.proof_quality)
            .encode(rgb.as_raw(), width, height, ExtendedColorType::Rgb8)
            .map_err(ProofError::Encode)?;

        Ok(Proof {
            bytes,
            width,
            height,
        })
    }
}

/// Leaves pixels untouched when the profile is unusable: an unreadable profile
/// must not make an otherwise valid photo unprocessable.
fn convert_to_srgb(rgb: &mut RgbImage, icc_profile: &[u8]) {
    let Ok(source) = ColorProfile::new_from_slice(icc_profile) else {
        return;
    };
    let Ok(transform) = source.create_transform_8bit(
        Layout::Rgb,
        &ColorProfile::new_srgb(),
        Layout::Rgb,
        TransformOptions::default(),
    ) else {
        return;
    };

    let mut converted = vec![0; rgb.as_raw().len()];
    if transform.transform(rgb.as_raw(), &mut converted).is_ok() {
        rgb.copy_from_slice(&converted);
    }
}
