use strum::{Display, EnumIter};

const TAG_MAKE: u16 = 0x010F;
const TAG_DNG_VERSION: u16 = 0xC612;
const TYPE_ASCII: u16 = 2;
const IFD_ENTRY_LEN: usize = 12;
const RAF_MAGIC: &[u8] = b"FUJIFILMCCD-RAW ";

/// Formats accepted at the upload boundary.
#[derive(Display, EnumIter, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[strum(serialize_all = "lowercase")]
pub enum UploadFormat {
    Jpeg,
    Cr2,
    Cr3,
    Nef,
    Arw,
    Dng,
    Raf,
}

impl UploadFormat {
    /// Identifies the format from file content alone.
    ///
    /// Only the first bytes are inspected, so this proves the container
    /// header, not that the rest of the file is a well-formed image. TIFF
    /// based RAW formats other than CR2 are told apart by the DNGVersion and
    /// Make tags of the first IFD, which must lie inside `header`.
    pub fn detect(header: &[u8]) -> Option<Self> {
        if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(Self::Jpeg);
        }
        if header.starts_with(RAF_MAGIC) {
            return Some(Self::Raf);
        }
        if header.get(4..12) == Some(b"ftypcrx ") {
            return Some(Self::Cr3);
        }

        detect_tiff_raw(header)
    }

    pub fn is_raw(self) -> bool {
        self != Self::Jpeg
    }

    /// Whether a client-declared `Content-Type` is plausible for this format.
    /// Parameters and case are ignored; an absent type is not a mismatch.
    pub fn accepts_content_type(self, declared: &str) -> bool {
        let declared = declared
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        if declared.is_empty() {
            return true;
        }

        let specific: &[&str] = match self {
            Self::Jpeg => return declared == "image/jpeg",
            Self::Cr2 => &["image/x-canon-cr2"],
            Self::Cr3 => &["image/x-canon-cr3"],
            Self::Nef => &["image/x-nikon-nef"],
            Self::Arw => &["image/x-sony-arw"],
            Self::Dng => &["image/x-adobe-dng", "image/dng"],
            Self::Raf => &["image/x-fuji-raf"],
        };
        let tiff_based = !matches!(self, Self::Cr3 | Self::Raf);

        declared == "application/octet-stream"
            || specific.contains(&declared.as_str())
            || (tiff_based && declared == "image/tiff")
    }

    /// Whether a lowercase client extension is plausible for this format.
    pub fn accepts_extension(self, extension: &str) -> bool {
        match self {
            Self::Jpeg => matches!(extension, "jpg" | "jpeg"),
            _ => extension == self.to_string(),
        }
    }
}

#[derive(Clone, Copy)]
enum ByteOrder {
    Little,
    Big,
}

struct Reader<'a> {
    data: &'a [u8],
    order: ByteOrder,
}

impl Reader<'_> {
    fn u16(&self, at: usize) -> Option<u16> {
        let bytes: [u8; 2] = self.data.get(at..at.checked_add(2)?)?.try_into().ok()?;

        Some(match self.order {
            ByteOrder::Little => u16::from_le_bytes(bytes),
            ByteOrder::Big => u16::from_be_bytes(bytes),
        })
    }

    fn u32(&self, at: usize) -> Option<u32> {
        let bytes: [u8; 4] = self.data.get(at..at.checked_add(4)?)?.try_into().ok()?;

        Some(match self.order {
            ByteOrder::Little => u32::from_le_bytes(bytes),
            ByteOrder::Big => u32::from_be_bytes(bytes),
        })
    }

    fn ascii_value(&self, entry: usize) -> Option<&[u8]> {
        if self.u16(entry + 2)? != TYPE_ASCII {
            return None;
        }
        let len = usize::try_from(self.u32(entry + 4)?).ok()?;
        let start = if len <= 4 {
            entry + 8
        } else {
            usize::try_from(self.u32(entry + 8)?).ok()?
        };
        let value = self.data.get(start..start.checked_add(len)?)?;

        Some(value.split(|&byte| byte == 0).next().unwrap_or_default())
    }
}

fn detect_tiff_raw(header: &[u8]) -> Option<UploadFormat> {
    let order = match header.get(..4)? {
        b"II*\0" => ByteOrder::Little,
        b"MM\0*" => ByteOrder::Big,
        _ => return None,
    };
    let reader = Reader {
        data: header,
        order,
    };

    // Canon CR2 stores a "CR" marker right after the TIFF header.
    if matches!(order, ByteOrder::Little) && header.get(8..10) == Some(b"CR") {
        return Some(UploadFormat::Cr2);
    }

    let ifd = usize::try_from(reader.u32(4)?).ok()?;
    let count = usize::from(reader.u16(ifd)?);
    let mut make: Option<&[u8]> = None;

    for index in 0..count {
        let entry = ifd
            .checked_add(2)?
            .checked_add(index.checked_mul(IFD_ENTRY_LEN)?)?;
        match reader.u16(entry)? {
            TAG_DNG_VERSION => return Some(UploadFormat::Dng),
            TAG_MAKE => make = reader.ascii_value(entry),
            _ => {}
        }
    }

    let make = make?.to_ascii_uppercase();
    if make.starts_with(b"NIKON") {
        Some(UploadFormat::Nef)
    } else if make.starts_with(b"SONY") {
        Some(UploadFormat::Arw)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;
    use crate::upload::fixtures;

    fn fixture(format: UploadFormat) -> Vec<u8> {
        match format {
            UploadFormat::Jpeg => fixtures::jpeg(10, 10),
            UploadFormat::Cr2 => fixtures::cr2(),
            UploadFormat::Cr3 => fixtures::cr3(),
            UploadFormat::Nef => fixtures::nef(),
            UploadFormat::Arw => fixtures::arw(),
            UploadFormat::Dng => fixtures::dng(),
            UploadFormat::Raf => fixtures::raf(),
        }
    }

    #[test]
    fn every_configured_format_is_detected_from_its_signature() {
        for format in UploadFormat::iter() {
            assert_eq!(
                UploadFormat::detect(&fixture(format)),
                Some(format),
                "{format}"
            );
        }
    }

    #[test]
    fn dng_tag_wins_over_make() {
        let header = fixtures::tiff(false, Some("SONY"), true);

        assert_eq!(UploadFormat::detect(&header), Some(UploadFormat::Dng));
    }

    #[test]
    fn plain_tiff_and_unknown_makes_are_rejected() {
        assert_eq!(
            UploadFormat::detect(&fixtures::tiff(false, None, false)),
            None
        );
        assert_eq!(
            UploadFormat::detect(&fixtures::tiff(true, Some("ACME"), false)),
            None
        );
    }

    #[test]
    fn other_file_types_are_rejected() {
        for header in [
            &b"\x89PNG\r\n\x1a\n"[..],
            b"GIF89a\x01\x00\x01\x00",
            b"%PDF-1.7 padding",
            b"<html><script>alert(1)</script>",
            b"PK\x03\x04 padding bytes",
            b"MZ\x90\x00 padding bytes",
        ] {
            assert_eq!(UploadFormat::detect(header), None);
        }
    }

    #[test]
    fn truncated_signatures_are_rejected() {
        let signature_lengths = [
            (UploadFormat::Raf, 16),
            (UploadFormat::Cr3, 12),
            (UploadFormat::Nef, fixtures::nef().len()),
            (UploadFormat::Arw, fixtures::arw().len()),
        ];

        for (format, signature_len) in signature_lengths {
            let full = fixture(format);

            for end in 0..signature_len {
                assert_ne!(
                    UploadFormat::detect(&full[..end]),
                    Some(format),
                    "{format} at {end}"
                );
            }
        }
        assert_eq!(UploadFormat::detect(&[0xFF, 0xD8]), None);
        assert_eq!(UploadFormat::detect(&[]), None);
    }

    #[test]
    fn tiff_with_out_of_range_offsets_is_rejected_without_panic() {
        let mut header = fixtures::arw();
        header[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(UploadFormat::detect(&header), None);

        // Make value offset pointing past the buffer.
        let mut header = fixtures::arw();
        header[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(UploadFormat::detect(&header), None);

        // Entry count larger than the data available.
        let mut header = fixtures::arw();
        header[8..10].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(UploadFormat::detect(&header), None);
    }

    #[test]
    fn content_type_must_fit_the_detected_format() {
        assert!(UploadFormat::Jpeg.accepts_content_type("image/jpeg"));
        assert!(UploadFormat::Jpeg.accepts_content_type("IMAGE/JPEG; charset=binary"));
        assert!(!UploadFormat::Jpeg.accepts_content_type("application/octet-stream"));
        assert!(!UploadFormat::Jpeg.accepts_content_type("text/html"));
        assert!(UploadFormat::Nef.accepts_content_type("image/x-nikon-nef"));
        assert!(UploadFormat::Nef.accepts_content_type("application/octet-stream"));
        assert!(UploadFormat::Dng.accepts_content_type("image/tiff"));
        assert!(!UploadFormat::Raf.accepts_content_type("image/tiff"));
        assert!(!UploadFormat::Nef.accepts_content_type("image/jpeg"));
        assert!(UploadFormat::Raf.accepts_content_type(""));
    }

    #[test]
    fn extension_must_fit_the_detected_format() {
        assert!(UploadFormat::Jpeg.accepts_extension("jpg"));
        assert!(UploadFormat::Jpeg.accepts_extension("jpeg"));
        assert!(!UploadFormat::Jpeg.accepts_extension("cr2"));
        assert!(UploadFormat::Cr3.accepts_extension("cr3"));
        assert!(!UploadFormat::Cr3.accepts_extension("cr2"));
    }
}
