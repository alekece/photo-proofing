//! Normalisation of the studio logo, so the worker can rely on its format.
//!
//! The upload may be a PNG or a JPEG of any dimensions. It is checked for size, signature and pixel
//! count before it is decoded, then shrunk while loading and re-encoded as an 8-bit sRGB PNG with
//! an alpha band, no metadata and a long edge of at most [`LogoNormaliser::MAX_EDGE`].

use libvips::{
    VipsImage,
    ops::{self, Interpretation, PngsaveBufferOptions},
};
use pp_core::config::DEFAULT_MAX_PIXELS;

use crate::{config::UploadConfig, error::ErrorResponse};

/// An image format accepted for the logo, recognised by its signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogoFormat {
    Png,
    Jpeg,
}

impl LogoFormat {
    const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    const JPEG_SIGNATURE: [u8; 3] = [0xff, 0xd8, 0xff];

    fn detect(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&Self::PNG_SIGNATURE) {
            Some(Self::Png)
        } else if bytes.starts_with(&Self::JPEG_SIGNATURE) {
            Some(Self::Jpeg)
        } else {
            None
        }
    }
}

/// Checks an uploaded logo against the operator's byte cap and the pixel safety cap.
pub(crate) struct LogoNormaliser {
    max_size: u64,
    max_pixels: u64,
}

impl From<&UploadConfig> for LogoNormaliser {
    fn from(config: &UploadConfig) -> Self {
        Self {
            max_size: config.logo_max_size.get(),
            max_pixels: DEFAULT_MAX_PIXELS.get(),
        }
    }
}

impl LogoNormaliser {
    /// Long edge of the stored logo.
    pub(crate) const MAX_EDGE: i32 = 512;

    /// The normalised PNG for an uploaded PNG or JPEG, decoded off the async runtime.
    pub(crate) async fn normalise(&self, upload: Vec<u8>) -> Result<Vec<u8>, ErrorResponse> {
        let (max_size, max_pixels) = (self.max_size, self.max_pixels);

        tokio::task::spawn_blocking(move || Self::render(&upload, max_size, max_pixels))
            .await
            .map_err(|error| {
                tracing::error!(%error, "logo normalisation task failed");
                ErrorResponse::unavailable()
            })?
    }

    fn render(upload: &[u8], max_size: u64, max_pixels: u64) -> Result<Vec<u8>, ErrorResponse> {
        if upload.len() as u64 > max_size {
            return Err(ErrorResponse::logo_too_large());
        }

        let format = LogoFormat::detect(upload).ok_or_else(ErrorResponse::invalid_logo)?;

        // Opening reads only the header, so the size is checked before any pixel is decoded.
        let header = VipsImage::new_from_buffer(upload, "[fail-on=error]")
            .map_err(|_| ErrorResponse::invalid_logo())?;
        let (width, height) = (header.get_width(), header.get_height());

        if width < 1 || height < 1 {
            return Err(ErrorResponse::invalid_logo());
        }

        if u64::from(width.unsigned_abs()) * u64::from(height.unsigned_abs()) > max_pixels {
            return Err(ErrorResponse::logo_too_large());
        }

        Self::encode(upload, format, width.max(height)).map_err(|_| ErrorResponse::invalid_logo())
    }

    /// The largest JPEG decoder shrink (1, 2, 4 or 8) that keeps the long edge at
    /// [`Self::MAX_EDGE`] or more, so a huge JPEG is never held at full size. PNG cannot shrink
    /// on load; the pixel cap bounds its decoded size.
    fn load_options(format: LogoFormat, long_edge: i32) -> String {
        match format {
            LogoFormat::Png => "[fail-on=error]".to_owned(),
            LogoFormat::Jpeg => {
                let shrink = [8, 4, 2]
                    .into_iter()
                    .find(|factor| long_edge / factor >= Self::MAX_EDGE)
                    .unwrap_or(1);

                format!("[fail-on=error,shrink={shrink}]")
            }
        }
    }

    fn encode(upload: &[u8], format: LogoFormat, long_edge: i32) -> libvips::Result<Vec<u8>> {
        let image = VipsImage::new_from_buffer(upload, &Self::load_options(format, long_edge))?;
        let image = ops::autorot(&image)?;
        let image = if matches!(image.get_interpretation(), Ok(Interpretation::Srgb)) {
            ops::copy(&image)?
        } else {
            ops::colourspace(&image, Interpretation::Srgb)?
        };
        let image = if image.image_hasalpha() {
            image
        } else {
            ops::bandjoin_const(&image, &mut [255.0])?
        };
        let long_edge = image.get_width().max(image.get_height());
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
    use libvips::ops::{black, jpegsave_buffer, pngsave_buffer};

    use super::*;
    use crate::upload::jpeg::{
        self,
        tests::{orientation_exif, with_segment},
    };

    fn png(width: i32, height: i32) -> Vec<u8> {
        jpeg::install().unwrap();

        pngsave_buffer(&black(width, height).unwrap()).unwrap()
    }

    fn jpeg_logo(width: i32, height: i32) -> Vec<u8> {
        jpeg::install().unwrap();

        jpegsave_buffer(&black(width, height).unwrap()).unwrap()
    }

    fn normaliser() -> LogoNormaliser {
        LogoNormaliser {
            max_size: 1024 * 1024,
            max_pixels: 1_000_000,
        }
    }

    fn decoded(bytes: &[u8]) -> VipsImage {
        VipsImage::new_from_buffer(bytes, "").unwrap()
    }

    #[tokio::test]
    async fn a_grey_png_becomes_an_rgba_one() {
        let normal = normaliser().normalise(png(40, 20)).await.unwrap();
        let image = decoded(&normal);

        assert_eq!((image.get_width(), image.get_height()), (40, 20));
        assert_eq!(image.get_bands(), 4);
    }

    #[tokio::test]
    async fn a_jpeg_becomes_an_rgba_png_too() {
        let normal = normaliser().normalise(jpeg_logo(40, 20)).await.unwrap();
        let image = decoded(&normal);

        assert!(normal.starts_with(&LogoFormat::PNG_SIGNATURE));
        assert_eq!((image.get_width(), image.get_height()), (40, 20));
        assert_eq!(image.get_bands(), 4);
    }

    #[tokio::test]
    async fn a_jpeg_logo_is_turned_upright() {
        let source = with_segment(&jpeg_logo(40, 20), 0xe1, &orientation_exif(6));
        let normal = normaliser().normalise(source).await.unwrap();
        let image = decoded(&normal);

        assert_eq!((image.get_width(), image.get_height()), (20, 40));
    }

    #[tokio::test]
    async fn a_large_logo_is_scaled_down_to_the_long_edge() {
        let normal = normaliser().normalise(png(1024, 256)).await.unwrap();
        let image = decoded(&normal);

        assert_eq!(image.get_width(), LogoNormaliser::MAX_EDGE);
        assert_eq!(image.get_height(), 128);
    }

    #[tokio::test]
    async fn anything_but_a_png_or_a_jpeg_is_refused() {
        for bytes in [
            Vec::new(),
            b"GIF89a....".to_vec(),
            vec![0x89, b'P', b'N', b'G'],
            vec![0xff, 0xd8],
        ] {
            let error = normaliser().normalise(bytes).await.unwrap_err();

            assert_eq!(
                serde_json::to_value(&error).unwrap()["code"],
                "invalid_logo"
            );
        }
    }

    #[tokio::test]
    async fn a_truncated_logo_is_refused() {
        let mut bytes = png(200, 200);

        bytes.truncate(bytes.len() / 2);

        let error = normaliser().normalise(bytes).await.unwrap_err();

        assert_eq!(
            serde_json::to_value(&error).unwrap()["code"],
            "invalid_logo"
        );
    }

    #[tokio::test]
    async fn a_logo_beyond_the_old_four_megapixel_limit_is_accepted_and_shrunk() {
        let normaliser = LogoNormaliser {
            max_size: 1024 * 1024,
            max_pixels: 100_000_000,
        };
        let normal = normaliser.normalise(png(2001, 2000)).await.unwrap();
        let image = decoded(&normal);

        assert_eq!(
            image.get_width().max(image.get_height()),
            LogoNormaliser::MAX_EDGE
        );
    }

    #[tokio::test]
    async fn too_many_pixels_or_bytes_are_refused() {
        let pixels = normaliser().normalise(png(1001, 1000)).await.unwrap_err();
        let mut bytes = png(10, 10);

        bytes.resize(1024 * 1024 + 1, 0);

        let size = normaliser().normalise(bytes).await.unwrap_err();

        assert_eq!(
            serde_json::to_value(&pixels).unwrap()["code"],
            "logo_too_large"
        );
        assert_eq!(
            serde_json::to_value(&size).unwrap()["code"],
            "logo_too_large"
        );
    }
}
