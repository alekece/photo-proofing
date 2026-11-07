//! Normalisation of the studio logo, so the worker can rely on its format.
//!
//! The upload is checked for size, signature and pixel count before it is decoded. It is then
//! re-encoded as an 8-bit sRGB PNG with an alpha band, no metadata and a long edge of at most
//! [`LogoNormaliser::MAX_EDGE`].

use libvips::{
    VipsImage,
    ops::{self, Interpretation, PngsaveBufferOptions},
};

use crate::error::ErrorResponse;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

pub(crate) struct LogoNormaliser;

impl LogoNormaliser {
    /// Largest accepted upload, in bytes.
    pub(crate) const MAX_BYTES: usize = 1024 * 1024;
    /// Most pixels a logo may declare before it is decoded.
    pub(crate) const MAX_PIXELS: i64 = 4_000_000;
    /// Long edge of the stored logo.
    pub(crate) const MAX_EDGE: i32 = 512;

    /// The normalised PNG for an uploaded one, decoded off the async runtime.
    pub(crate) async fn normalise(png: Vec<u8>) -> Result<Vec<u8>, ErrorResponse> {
        tokio::task::spawn_blocking(move || Self::render(&png))
            .await
            .map_err(|error| {
                tracing::error!(%error, "logo normalisation task failed");
                ErrorResponse::unavailable()
            })?
    }

    fn render(png: &[u8]) -> Result<Vec<u8>, ErrorResponse> {
        if png.len() > Self::MAX_BYTES {
            return Err(ErrorResponse::logo_too_large());
        }

        if !png.starts_with(&PNG_SIGNATURE) {
            return Err(ErrorResponse::invalid_logo());
        }

        let image = VipsImage::new_from_buffer(png, "[fail-on=error]")
            .map_err(|_| ErrorResponse::invalid_logo())?;
        let (width, height) = (image.get_width(), image.get_height());

        if width < 1 || height < 1 {
            return Err(ErrorResponse::invalid_logo());
        }

        if i64::from(width) * i64::from(height) > Self::MAX_PIXELS {
            return Err(ErrorResponse::logo_too_large());
        }

        Self::encode(&image, width.max(height)).map_err(|_| ErrorResponse::invalid_logo())
    }

    fn encode(image: &VipsImage, long_edge: i32) -> libvips::Result<Vec<u8>> {
        let image = if matches!(image.get_interpretation(), Ok(Interpretation::Srgb)) {
            ops::copy(image)?
        } else {
            ops::colourspace(image, Interpretation::Srgb)?
        };
        let image = if image.image_hasalpha() {
            image
        } else {
            ops::bandjoin_const(&image, &mut [255.0])?
        };
        let image = if long_edge > Self::MAX_EDGE {
            ops::resize(&image, f64::from(Self::MAX_EDGE) / f64::from(long_edge))?
        } else {
            image
        };

        ops::pngsave_buffer_with_opts(
            &image,
            &PngsaveBufferOptions {
                keep: ops::ForeignKeep::None,
                profile: None,
                ..PngsaveBufferOptions::default()
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use libvips::ops::{black, pngsave_buffer};

    use super::*;
    use crate::upload::jpeg;

    fn png(width: i32, height: i32) -> Vec<u8> {
        jpeg::install().unwrap();

        pngsave_buffer(&black(width, height).unwrap()).unwrap()
    }

    fn decoded(bytes: &[u8]) -> VipsImage {
        VipsImage::new_from_buffer(bytes, "").unwrap()
    }

    #[tokio::test]
    async fn a_grey_png_becomes_an_rgba_one() {
        let normal = LogoNormaliser::normalise(png(40, 20)).await.unwrap();
        let image = decoded(&normal);

        assert_eq!((image.get_width(), image.get_height()), (40, 20));
        assert_eq!(image.get_bands(), 4);
    }

    #[tokio::test]
    async fn a_large_logo_is_scaled_down_to_the_long_edge() {
        let normal = LogoNormaliser::normalise(png(1024, 256)).await.unwrap();
        let image = decoded(&normal);

        assert_eq!(image.get_width(), LogoNormaliser::MAX_EDGE);
        assert_eq!(image.get_height(), 128);
    }

    #[tokio::test]
    async fn anything_but_a_png_is_refused() {
        for bytes in [
            Vec::new(),
            b"GIF89a....".to_vec(),
            vec![0x89, b'P', b'N', b'G'],
        ] {
            let error = LogoNormaliser::normalise(bytes).await.unwrap_err();

            assert_eq!(error.to_string(), "invalid PNG logo");
        }
    }

    #[tokio::test]
    async fn a_truncated_png_is_refused() {
        let mut bytes = png(200, 200);

        bytes.truncate(bytes.len() / 2);

        let error = LogoNormaliser::normalise(bytes).await.unwrap_err();

        assert_eq!(error.to_string(), "invalid PNG logo");
    }

    #[tokio::test]
    async fn too_many_pixels_or_bytes_are_refused() {
        let pixels = LogoNormaliser::normalise(png(2001, 2000))
            .await
            .unwrap_err();
        let mut bytes = png(10, 10);

        bytes.resize(LogoNormaliser::MAX_BYTES + 1, 0);

        let size = LogoNormaliser::normalise(bytes).await.unwrap_err();

        assert_eq!(pixels.to_string(), "logo is too large");
        assert_eq!(size.to_string(), "logo is too large");
    }
}
