//! Proof derivatives rendered from an original JPEG with libvips.
//!
//! Rendering is blocking; callers run it through `spawn_blocking`. It needs libvips 8.15 or newer,
//! the first release with the `keep` option that the typed save always sends.

use std::{
    num::NonZeroU64,
    sync::{Mutex, OnceLock},
};

use libvips::{
    VipsApp, VipsImage,
    ops::{self, ForeignKeep, Interpretation, JpegsaveBufferOptions},
};
use pp_database::watermark::Watermark;
use thiserror::Error;

use crate::watermark::WatermarkPainter;

/// Long edge of the gallery grid thumbnail.
const THUMBNAIL_EDGE: u32 = 400;
/// Long edge of the full-screen preview: sharp on laptops and phones, too small to print.
const PREVIEW_EDGE: u32 = 1600;
/// JPEG output quality.
const JPEG_QUALITY: i32 = 75;
/// JPEG decoder downscale factors, largest first.
const JPEG_SHRINKS: [u32; 3] = [8, 4, 2];
/// Metadata field holding an embedded ICC profile.
const ICC_PROFILE: &str = "icc-profile-data";

static VIPS_APP: OnceLock<Mutex<Option<VipsApp>>> = OnceLock::new();

/// Failure to render derivatives.
#[derive(Debug, Error)]
pub enum RenderError {
    /// The original cannot be decoded; retrying cannot help.
    #[error("the original photo cannot be decoded")]
    Unreadable,
    /// The original has more pixels than the worker accepts; retrying cannot help.
    #[error("the original photo has too many pixels")]
    TooLarge,
    /// The image library itself failed.
    #[error("image library failure: {0}")]
    Library(String),
}

/// Encoded JPEG derivatives of one original.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPhoto {
    pub thumbnail: Vec<u8>,
    pub preview: Vec<u8>,
}

/// Proof outputs; clean copies are omitted during marked-only regeneration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedProof {
    pub clean: Option<RenderedPhoto>,
    pub watermarked: RenderedPhoto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofRender {
    All,
    Watermarked,
}

/// Renders proof derivatives from JPEG originals of at most `max_pixels`.
#[derive(Debug, Clone, Copy)]
pub struct ImageRenderer {
    max_pixels: NonZeroU64,
}

impl ImageRenderer {
    /// Initializes libvips once for the process, with caches off and one thread per operation.
    pub fn new(max_pixels: NonZeroU64) -> Result<Self, RenderError> {
        let app = VIPS_APP.get_or_init(|| Mutex::new(None));
        let mut app = app
            .lock()
            .map_err(|_| RenderError::Library("initialization state is poisoned".to_owned()))?;

        if app.is_none() {
            let initialized = VipsApp::new("photo-proofing-worker", false)
                .map_err(|error| RenderError::Library(format!("{error:?}")))?;

            initialized.concurrency_set(1);
            initialized.cache_set_max(0);
            initialized.cache_set_max_mem(0);
            initialized.cache_set_max_files(0);
            *app = Some(initialized);
        }

        Ok(Self { max_pixels })
    }

    /// Renders the thumbnail and preview of `original`, with `watermark` drawn on both.
    ///
    /// Outputs are auto-rotated, sRGB, stripped of metadata and never larger than the original.
    /// A truncated or otherwise undecodable original is [`RenderError::Unreadable`].
    pub fn render(
        &self,
        original: &[u8],
        watermark: Option<&Watermark>,
    ) -> Result<RenderedPhoto, RenderError> {
        let image = self.prepare(original)?;

        Ok(RenderedPhoto {
            thumbnail: Self::fit(&image, THUMBNAIL_EDGE, watermark)?,
            preview: Self::fit(&image, PREVIEW_EDGE, watermark)?,
        })
    }

    /// Encodes both pairs for [`ProofRender::All`], or only the marked pair for
    /// [`ProofRender::Watermarked`], from resized source pixels, not compressed clean copies.
    pub fn render_proof(
        &self,
        original: &[u8],
        watermark: Option<&Watermark>,
        render: ProofRender,
    ) -> Result<RenderedProof, RenderError> {
        let image = self.prepare(original)?;
        let thumbnail = Self::fit_pair(&image, THUMBNAIL_EDGE, watermark, render)?;
        let preview = Self::fit_pair(&image, PREVIEW_EDGE, watermark, render)?;
        let clean = thumbnail
            .0
            .zip(preview.0)
            .map(|(thumbnail, preview)| RenderedPhoto { thumbnail, preview });

        Ok(RenderedProof {
            clean,
            watermarked: RenderedPhoto {
                thumbnail: thumbnail.1,
                preview: preview.1,
            },
        })
    }

    fn prepare(&self, original: &[u8]) -> Result<VipsImage, RenderError> {
        // Loading is lazy: this reads the header only.
        let header =
            VipsImage::new_from_buffer(original, "").map_err(|_| RenderError::Unreadable)?;

        if header.get_as_string("vips-loader").ok().as_deref() != Some("jpegload_buffer") {
            return Err(RenderError::Unreadable);
        }

        let width = u32::try_from(header.get_width()).map_err(|_| RenderError::Unreadable)?;
        let height = u32::try_from(header.get_height()).map_err(|_| RenderError::Unreadable)?;

        if u64::from(width) * u64::from(height) > self.max_pixels.get() {
            return Err(RenderError::TooLarge);
        }

        // Decode at the smallest scale that still covers the preview, so a 100 MP original never
        // occupies full-resolution memory.
        let long_edge = width.max(height);
        let shrink = JPEG_SHRINKS
            .into_iter()
            .find(|shrink| long_edge / shrink >= PREVIEW_EDGE)
            .unwrap_or(1);
        // The typed `jpegload_buffer_with_opts` passes the output-only `flags` argument as an input,
        // which crashes libvips 8.15, so the loader takes an option string.
        let image =
            VipsImage::new_from_buffer(original, &format!("shrink={shrink},fail-on=truncated"))
                .map_err(|_| RenderError::Unreadable)?;
        // `thumbnail_image` ignores embedded RGB profiles, so wide-gamut sources (Display P3,
        // Adobe RGB) are converted here. A profile libvips rejects leaves the pixels as stored.
        let image = if image.get_as_string(ICC_PROFILE).is_ok() {
            ops::icc_transform(&image, "srgb").unwrap_or(image)
        } else {
            image
        };

        Ok(image)
    }

    /// Encodes `image` with its long edge at most `edge`, without upscaling.
    ///
    /// Decoding happens here, so every failure means the original is unreadable: the bindings only
    /// report static messages, which cannot tell a corrupt stream from a library fault.
    fn fit(
        image: &VipsImage,
        edge: u32,
        watermark: Option<&Watermark>,
    ) -> Result<Vec<u8>, RenderError> {
        let fitted = Self::resize(image, edge)?;

        let fitted = match watermark {
            Some(watermark) => WatermarkPainter::new(watermark).paint(&fitted)?,
            None => fitted,
        };

        Self::encode(&fitted)
    }

    fn fit_pair(
        image: &VipsImage,
        edge: u32,
        watermark: Option<&Watermark>,
        render: ProofRender,
    ) -> Result<(Option<Vec<u8>>, Vec<u8>), RenderError> {
        let fitted = Self::resize(image, edge)?;

        let clean = if render == ProofRender::All {
            Some(Self::encode(&fitted)?)
        } else {
            None
        };

        let marked = match watermark {
            Some(watermark) => Self::encode(&WatermarkPainter::new(watermark).paint(&fitted)?)?,
            None => match &clean {
                Some(clean) => clean.clone(),
                None => Self::encode(&fitted)?,
            },
        };

        Ok((clean, marked))
    }

    fn resize(image: &VipsImage, edge: u32) -> Result<VipsImage, RenderError> {
        let long_edge = image.get_width().max(image.get_height());
        let target = long_edge.min(i32::try_from(edge).unwrap_or(i32::MAX));
        // `thumbnail_image` applies the EXIF orientation.
        let mut fitted =
            ops::thumbnail_image(image, target).map_err(|_| RenderError::Unreadable)?;

        if !matches!(fitted.get_interpretation(), Ok(Interpretation::Srgb)) {
            fitted = ops::colourspace(&fitted, Interpretation::Srgb)
                .map_err(|_| RenderError::Unreadable)?;
        }

        Ok(fitted)
    }

    fn encode(fitted: &VipsImage) -> Result<Vec<u8>, RenderError> {
        // No `keep` flag drops EXIF, XMP and IPTC; the binding always sends a null `profile`, which
        // would write the image's ICC profile back, so `"none"` says to embed none.
        ops::jpegsave_buffer_with_opts(
            fitted,
            &JpegsaveBufferOptions {
                q: JPEG_QUALITY,
                keep: ForeignKeep::None,
                profile: Some("none".to_owned()),
                ..JpegsaveBufferOptions::default()
            },
        )
        .map_err(|_| RenderError::Unreadable)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use libvips::{
        VipsImage,
        ops::{BandFormat, Interpretation, black, colourspace, gaussnoise, jpegsave_buffer},
    };

    use super::*;

    const UNBOUNDED: NonZeroU64 = NonZeroU64::MAX;

    /// Initializes libvips for tests that build images before creating any renderer.
    pub(crate) fn init_libvips() {
        ImageRenderer::new(UNBOUNDED).expect("libvips should initialize");
    }

    fn jpeg(width: i32, height: i32) -> Vec<u8> {
        init_libvips();
        jpegsave_buffer(&black(width, height).unwrap()).unwrap()
    }

    /// Inserts an application segment right after the start-of-image marker.
    fn with_segment(jpeg: &[u8], marker: u8, payload: &[u8]) -> Vec<u8> {
        let length = u16::try_from(payload.len() + 2).unwrap();
        let mut out = jpeg[..2].to_vec();

        out.extend([0xff, marker]);
        out.extend(length.to_be_bytes());
        out.extend(payload);
        out.extend(&jpeg[2..]);

        out
    }

    /// Big-endian EXIF block: one IFD0 entry, plus an optional GPS IFD.
    fn exif(ifd0_tag: u16, ifd0_type: u16, ifd0_value: u32, gps: bool) -> Vec<u8> {
        let mut exif = b"Exif\0\0MM\0\x2a\0\0\0\x08".to_vec();

        exif.extend(1_u16.to_be_bytes());
        exif.extend(ifd0_tag.to_be_bytes());
        exif.extend(ifd0_type.to_be_bytes());
        exif.extend(1_u32.to_be_bytes());
        exif.extend(ifd0_value.to_be_bytes());
        exif.extend(0_u32.to_be_bytes());

        if gps {
            // GPSLatitudeRef = "N", in the GPS IFD that IFD0 points at (offset 26).
            exif.extend(1_u16.to_be_bytes());
            exif.extend([0, 1, 0, 2, 0, 0, 0, 2, b'N', 0, 0, 0]);
            exif.extend(0_u32.to_be_bytes());
        }

        exif
    }

    /// Markers of the segments before the scan.
    fn markers(jpeg: &[u8]) -> Vec<u8> {
        let mut markers = Vec::new();
        let mut offset = 2;

        while jpeg[offset] == 0xff && jpeg[offset + 1] != 0xda {
            let length = usize::from(u16::from_be_bytes([jpeg[offset + 2], jpeg[offset + 3]]));

            markers.push(jpeg[offset + 1]);
            offset += 2 + length;
        }

        markers
    }

    fn assert_clean(jpeg: &[u8]) {
        let markers = markers(jpeg);

        // APP1 holds EXIF and XMP, APP2 ICC profiles, APP13 IPTC.
        for marker in [0xe1, 0xe2, 0xed] {
            assert!(
                !markers.contains(&marker),
                "marker {marker:#x} in {markers:x?}"
            );
        }
    }

    fn size(jpeg: &[u8]) -> (i32, i32) {
        let image = VipsImage::new_from_buffer(jpeg, "").unwrap();

        (image.get_width(), image.get_height())
    }

    fn render(original: &[u8], max_pixels: NonZeroU64) -> Result<RenderedPhoto, RenderError> {
        ImageRenderer::new(max_pixels)
            .unwrap()
            .render(original, None)
    }

    #[test]
    fn proof_render_keeps_clean_and_marked_pairs_separate() {
        let renderer = ImageRenderer::new(UNBOUNDED).unwrap();
        let watermark = pp_database::watermark::WorkspaceSettings {
            studio_name: "Studio".to_owned(),
            ..pp_database::watermark::WorkspaceSettings::default()
        }
        .watermark()
        .unwrap();
        let source = jpeg(600, 400);
        let result = renderer
            .render_proof(&source, Some(&watermark), ProofRender::All)
            .unwrap();
        let clean = result.clean.unwrap();

        assert_eq!(size(&clean.thumbnail), (400, 267));
        assert_eq!(size(&clean.preview), (600, 400));
        assert_ne!(clean.thumbnail, result.watermarked.thumbnail);
        assert_ne!(clean.preview, result.watermarked.preview);
        assert_clean(&result.watermarked.thumbnail);
        assert_clean(&result.watermarked.preview);

        let marked_only = renderer
            .render_proof(&source, Some(&watermark), ProofRender::Watermarked)
            .unwrap();

        assert!(marked_only.clean.is_none());
        assert_eq!(marked_only.watermarked, result.watermarked);
    }

    #[test]
    fn unconfigured_watermark_produces_identical_pairs() {
        let result = ImageRenderer::new(UNBOUNDED)
            .unwrap()
            .render_proof(&jpeg(100, 80), None, ProofRender::All)
            .unwrap();

        assert_eq!(result.clean.unwrap(), result.watermarked);
    }

    #[test]
    fn output_fits_the_long_edge_and_keeps_aspect_ratio() {
        let small = render(&jpeg(300, 200), UNBOUNDED).unwrap();
        let large = render(&jpeg(4000, 2000), UNBOUNDED).unwrap();
        let portrait = render(&jpeg(1000, 2000), UNBOUNDED).unwrap();

        assert_eq!(size(&small.thumbnail), (300, 200));
        assert_eq!(size(&small.preview), (300, 200));
        assert_eq!(size(&large.thumbnail), (400, 200));
        assert_eq!(size(&large.preview), (1600, 800));
        assert_eq!(size(&portrait.thumbnail), (200, 400));
        assert_eq!(size(&portrait.preview), (800, 1600));
    }

    #[test]
    fn exif_orientation_is_applied() {
        // Orientation 6: the camera was turned, so the stored 400×200 displays as 200×400.
        let source = with_segment(&jpeg(400, 200), 0xe1, &exif(0x0112, 3, 6 << 16, false));

        assert_eq!(
            VipsImage::new_from_buffer(&source, "")
                .unwrap()
                .get_orientation(),
            6
        );

        let rendered = render(&source, UNBOUNDED).unwrap();

        assert_eq!(size(&rendered.thumbnail), (200, 400));
        assert_eq!(size(&rendered.preview), (200, 400));
        assert_clean(&rendered.preview);
    }

    #[test]
    fn metadata_is_stripped() {
        let xmp = b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta xmlns:x='adobe:ns:meta/'/>";
        let source = with_segment(&jpeg(40, 20), 0xe1, &exif(0x8825, 4, 26, true));
        let source = with_segment(&source, 0xe1, xmp);

        assert!(
            VipsImage::new_from_buffer(&source, "")
                .unwrap()
                .get_as_string("exif-ifd3-GPSLatitudeRef")
                .unwrap()
                .starts_with('N')
        );

        let rendered = render(&source, UNBOUNDED).unwrap();

        assert_clean(&rendered.thumbnail);
        assert_clean(&rendered.preview);
    }

    #[test]
    fn non_srgb_source_is_converted_to_srgb() {
        init_libvips();

        let srgb = colourspace(&black(40, 20).unwrap(), Interpretation::Srgb).unwrap();
        let cmyk = jpegsave_buffer(&colourspace(&srgb, Interpretation::Cmyk).unwrap()).unwrap();

        assert!(matches!(
            VipsImage::new_from_buffer(&cmyk, "")
                .unwrap()
                .get_interpretation()
                .unwrap(),
            Interpretation::Cmyk
        ));

        let rendered = render(&cmyk, UNBOUNDED).unwrap();

        for output in [rendered.thumbnail, rendered.preview] {
            let image = VipsImage::new_from_buffer(&output, "").unwrap();

            assert_eq!(image.get_bands(), 3);
            assert!(matches!(
                image.get_interpretation().unwrap(),
                Interpretation::Srgb
            ));
            assert_clean(&output);
        }
    }

    #[test]
    fn embedded_rgb_profile_is_converted_to_srgb() {
        init_libvips();

        let pixels = [200_u8, 60, 60].repeat(40 * 20);
        let image = VipsImage::new_from_memory_copy(&pixels, 40, 20, 3, BandFormat::Uchar).unwrap();
        let tagged = with_segment(
            &jpegsave_buffer(&image).unwrap(),
            0xe2,
            &[b"ICC_PROFILE\0\x01\x01".as_slice(), &swapped_profile()].concat(),
        );

        let rendered = render(&tagged, UNBOUNDED).unwrap();

        for output in [rendered.thumbnail, rendered.preview] {
            let decoded = VipsImage::new_from_buffer(&output, "").unwrap();
            let pixel = &decoded.image_write_to_memory()[..3];

            // The profile maps the source's red channel to sRGB green.
            assert!(pixel[1] > pixel[0] + 50, "pixel {pixel:?}");
            assert_clean(&output);
        }
    }

    /// ICC v2 RGB profile with sRGB primaries whose red and green are swapped.
    fn swapped_profile() -> Vec<u8> {
        fn s15(value: f64) -> [u8; 4] {
            ((value * 65536.0).round() as i32).to_be_bytes()
        }

        fn xyz(x: f64, y: f64, z: f64) -> Vec<u8> {
            [b"XYZ \0\0\0\0".as_slice(), &s15(x), &s15(y), &s15(z)].concat()
        }

        let white = xyz(0.9642, 1.0, 0.8249);
        let gamma = b"curv\0\0\0\0\0\0\0\x01\x02\x33\0\0".to_vec();
        let tags: [(&[u8; 4], Vec<u8>); 5] = [
            (b"wtpt", white.clone()),
            (b"rXYZ", xyz(0.3851, 0.7169, 0.0971)),
            (b"gXYZ", xyz(0.4361, 0.2225, 0.0139)),
            (b"bXYZ", xyz(0.1431, 0.0606, 0.7141)),
            (b"rTRC", gamma),
        ];
        let table = 4 + 12 * (tags.len() + 2);
        let mut entries = Vec::new();
        let mut data = Vec::new();

        for (signature, tag) in &tags {
            entries.extend(*signature);
            entries.extend(
                u32::try_from(128 + table + data.len())
                    .unwrap()
                    .to_be_bytes(),
            );
            entries.extend(u32::try_from(tag.len()).unwrap().to_be_bytes());
            data.extend(tag);
        }

        // The green and blue curves share the red one.
        let curve = &entries[entries.len() - 8..].to_vec();

        for signature in [b"gTRC", b"bTRC"] {
            entries.extend(signature);
            entries.extend(curve);
        }

        let size = u32::try_from(128 + table + data.len()).unwrap();
        let mut header = vec![0_u8; 128];

        header[..4].copy_from_slice(&size.to_be_bytes());
        header[8..12].copy_from_slice(&[2, 0x10, 0, 0]);
        header[12..24].copy_from_slice(b"mntrRGB XYZ ");
        header[36..40].copy_from_slice(b"acsp");
        header[68..80].copy_from_slice(&white[8..]);

        [
            header,
            u32::try_from(tags.len() + 2)
                .unwrap()
                .to_be_bytes()
                .to_vec(),
            entries,
            data,
        ]
        .concat()
    }

    #[test]
    fn corrupt_input_is_unreadable() {
        for input in [b"not a JPEG".as_slice(), b"\xff\xd8\xff\xe0garbage", b""] {
            assert!(matches!(
                render(input, UNBOUNDED),
                Err(RenderError::Unreadable)
            ));
        }
    }

    #[test]
    fn truncated_jpeg_is_unreadable() {
        init_libvips();

        let noisy = jpegsave_buffer(&gaussnoise(600, 400).unwrap()).unwrap();

        assert!(render(&noisy, UNBOUNDED).is_ok());
        assert!(matches!(
            render(&noisy[..noisy.len() / 2], UNBOUNDED),
            Err(RenderError::Unreadable)
        ));
    }

    #[test]
    fn oversized_dimensions_are_too_large_without_decoding() {
        init_libvips();

        let source = jpegsave_buffer(&gaussnoise(300, 200).unwrap()).unwrap();

        assert!(render(&source, NonZeroU64::new(60_000).unwrap()).is_ok());
        assert!(matches!(
            render(&source, NonZeroU64::new(59_999).unwrap()),
            Err(RenderError::TooLarge)
        ));
        // Only the header is valid: the bound must reject it before any pixel is decoded.
        assert!(matches!(
            render(
                &source[..source.len() / 2],
                NonZeroU64::new(59_999).unwrap()
            ),
            Err(RenderError::TooLarge)
        ));
    }
}
