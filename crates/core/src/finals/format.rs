use std::cmp::Ordering;

use imagesize::ImageType;

use super::error::FinalError;

const JPEG_SIGNATURE: &[u8] = &[0xFF, 0xD8, 0xFF];
const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
const TIFF_LITTLE_ENDIAN: &[u8] = &[b'I', b'I', 0x2A, 0x00];
const TIFF_BIG_ENDIAN: &[u8] = &[b'M', b'M', 0x00, 0x2A];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Tiff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Landscape,
    Portrait,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl ImageFormat {
    /// Detected from the file signature only; declared types are never trusted.
    pub fn detect(bytes: &[u8]) -> Result<Self, FinalError> {
        if bytes.starts_with(JPEG_SIGNATURE) {
            Ok(Self::Jpeg)
        } else if bytes.starts_with(PNG_SIGNATURE) {
            Ok(Self::Png)
        } else if bytes.starts_with(TIFF_LITTLE_ENDIAN) || bytes.starts_with(TIFF_BIG_ENDIAN) {
            Ok(Self::Tiff)
        } else {
            Err(FinalError::UnsupportedFormat)
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Tiff => "image/tiff",
        }
    }

    pub fn matches_mime(self, declared: &str) -> bool {
        let essence = declared.split(';').next().unwrap_or_default().trim();

        essence.eq_ignore_ascii_case(self.mime())
    }

    pub fn matches_extension(self, extension: &str) -> bool {
        let extension = extension.to_ascii_lowercase();

        match self {
            Self::Jpeg => matches!(extension.as_str(), "jpg" | "jpeg"),
            Self::Png => extension == "png",
            Self::Tiff => matches!(extension.as_str(), "tif" | "tiff"),
        }
    }

    /// Reads dimensions from the header without decoding pixels.
    pub fn read_dimensions(self, bytes: &[u8]) -> Result<Dimensions, FinalError> {
        let parsed_type = imagesize::image_type(bytes).map_err(|_| FinalError::InvalidHeader)?;
        let consistent = matches!(
            (self, parsed_type),
            (Self::Jpeg, ImageType::Jpeg)
                | (Self::Png, ImageType::Png)
                | (Self::Tiff, ImageType::Tiff)
        );

        if !consistent {
            return Err(FinalError::InvalidHeader);
        }

        let size = imagesize::blob_size(bytes).map_err(|_| FinalError::InvalidHeader)?;
        let width = u32::try_from(size.width).map_err(|_| FinalError::InvalidHeader)?;
        let height = u32::try_from(size.height).map_err(|_| FinalError::InvalidHeader)?;

        if width == 0 || height == 0 {
            return Err(FinalError::InvalidHeader);
        }

        Ok(Dimensions { width, height })
    }
}

impl Dimensions {
    pub fn long_edge(self) -> u32 {
        self.width.max(self.height)
    }

    pub fn pixels(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub fn orientation(self) -> Orientation {
        match self.width.cmp(&self.height) {
            Ordering::Greater => Orientation::Landscape,
            Ordering::Less => Orientation::Portrait,
            Ordering::Equal => Orientation::Square,
        }
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    pub fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = super::PNG_SIGNATURE.to_vec();
        bytes.extend(13u32.to_be_bytes());
        bytes.extend(b"IHDR");
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes.extend([8, 2, 0, 0, 0]);
        bytes.extend([0; 4]);

        bytes
    }

    pub fn jpeg(width: u16, height: u16) -> Vec<u8> {
        let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08];
        bytes.extend(height.to_be_bytes());
        bytes.extend(width.to_be_bytes());
        bytes.extend([0x03, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);

        bytes
    }

    pub fn tiff(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = vec![b'I', b'I', 0x2A, 0x00, 8, 0, 0, 0, 2, 0];
        for (tag, value) in [(256u16, width), (257, height)] {
            bytes.extend(tag.to_le_bytes());
            bytes.extend(4u16.to_le_bytes());
            bytes.extend(1u32.to_le_bytes());
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend([0; 4]);

        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::{fixtures, *};

    #[test]
    fn formats_are_detected_by_signature() {
        assert_eq!(
            ImageFormat::detect(&fixtures::jpeg(3, 2)),
            Ok(ImageFormat::Jpeg)
        );
        assert_eq!(
            ImageFormat::detect(&fixtures::png(3, 2)),
            Ok(ImageFormat::Png)
        );
        assert_eq!(
            ImageFormat::detect(&fixtures::tiff(3, 2)),
            Ok(ImageFormat::Tiff)
        );
        assert_eq!(
            ImageFormat::detect(b"GIF89a...."),
            Err(FinalError::UnsupportedFormat)
        );
        assert_eq!(ImageFormat::detect(b""), Err(FinalError::UnsupportedFormat));
    }

    #[test]
    fn dimensions_are_read_from_headers() {
        let expected = Dimensions {
            width: 300,
            height: 200,
        };

        assert_eq!(
            ImageFormat::Jpeg.read_dimensions(&fixtures::jpeg(300, 200)),
            Ok(expected)
        );
        assert_eq!(
            ImageFormat::Png.read_dimensions(&fixtures::png(300, 200)),
            Ok(expected)
        );
        assert_eq!(
            ImageFormat::Tiff.read_dimensions(&fixtures::tiff(300, 200)),
            Ok(expected)
        );
    }

    #[test]
    fn truncated_or_zero_sized_headers_are_rejected() {
        let png = fixtures::png(3, 2);

        assert_eq!(
            ImageFormat::Png.read_dimensions(&png[..12]),
            Err(FinalError::InvalidHeader)
        );
        assert_eq!(
            ImageFormat::Jpeg.read_dimensions(&[0xFF, 0xD8, 0xFF]),
            Err(FinalError::InvalidHeader)
        );
        assert_eq!(
            ImageFormat::Png.read_dimensions(&fixtures::png(0, 2)),
            Err(FinalError::InvalidHeader)
        );
    }

    #[test]
    fn orientation_follows_dimensions() {
        let of = |width, height| Dimensions { width, height }.orientation();

        assert_eq!(of(3, 2), Orientation::Landscape);
        assert_eq!(of(2, 3), Orientation::Portrait);
        assert_eq!(of(2, 2), Orientation::Square);
    }

    #[test]
    fn declared_mime_ignores_parameters_and_case() {
        assert!(ImageFormat::Jpeg.matches_mime("Image/JPEG; charset=binary"));
        assert!(!ImageFormat::Jpeg.matches_mime("image/png"));
    }
}
