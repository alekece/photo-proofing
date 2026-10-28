use crate::upload::error::UploadError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    pub fn pixels(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

/// Reads the frame size from the first start-of-frame marker, walking segment
/// lengths only. Nothing is decoded, so cost is bounded by the header size.
pub fn read_dimensions(data: &[u8]) -> Result<Dimensions, UploadError> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return Err(UploadError::MalformedImage);
    }

    let mut pos = 2;
    loop {
        // Markers may be preceded by any number of 0xFF fill bytes.
        while data.get(pos) == Some(&0xFF) && data.get(pos + 1) == Some(&0xFF) {
            pos += 1;
        }
        if data.get(pos) != Some(&0xFF) {
            return Err(UploadError::MalformedImage);
        }
        let marker = *data.get(pos + 1).ok_or(UploadError::MalformedImage)?;
        pos += 2;

        match marker {
            // Standalone markers carry no length.
            0x01 | 0xD0..=0xD7 => continue,
            // EOI or start of scan before any frame header.
            0xD9 | 0xDA | 0x00 => return Err(UploadError::MalformedImage),
            _ => {}
        }

        let length = data
            .get(pos..pos + 2)
            .map(|bytes| usize::from(u16::from_be_bytes([bytes[0], bytes[1]])))
            .ok_or(UploadError::MalformedImage)?;
        if length < 2 {
            return Err(UploadError::MalformedImage);
        }

        if is_start_of_frame(marker) {
            let frame = data
                .get(pos + 2..pos + length)
                .filter(|frame| frame.len() >= 6)
                .ok_or(UploadError::MalformedImage)?;
            let height = u32::from(u16::from_be_bytes([frame[1], frame[2]]));
            let width = u32::from(u16::from_be_bytes([frame[3], frame[4]]));
            if width == 0 || height == 0 {
                return Err(UploadError::MalformedImage);
            }

            return Ok(Dimensions { width, height });
        }

        pos += length;
    }
}

/// SOF0..SOF15 except DHT (C4), JPG (C8) and DAC (CC).
fn is_start_of_frame(marker: u8) -> bool {
    matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upload::fixtures;

    #[test]
    fn reads_width_and_height_from_the_frame_header() {
        let dimensions = read_dimensions(&fixtures::jpeg(6000, 4000)).expect("valid header");

        assert_eq!(
            dimensions,
            Dimensions {
                width: 6000,
                height: 4000
            }
        );
        assert_eq!(dimensions.pixels(), 24_000_000);
    }

    #[test]
    fn maximum_dimensions_do_not_overflow() {
        let dimensions = read_dimensions(&fixtures::jpeg(u16::MAX, u16::MAX)).expect("valid");

        assert_eq!(dimensions.pixels(), 65_535 * 65_535);
    }

    #[test]
    fn rejects_missing_soi() {
        assert_eq!(
            read_dimensions(b"\x00\x01\x02\x03"),
            Err(UploadError::MalformedImage)
        );
    }

    #[test]
    fn rejects_truncation_at_every_prefix() {
        let full = fixtures::jpeg(10, 10);
        let sof_end = full.len() - 2;

        for end in 0..sof_end {
            assert_eq!(
                read_dimensions(&full[..end]),
                Err(UploadError::MalformedImage),
                "prefix of {end} bytes"
            );
        }
    }

    #[test]
    fn rejects_zero_dimensions() {
        assert_eq!(
            read_dimensions(&fixtures::jpeg(0, 10)),
            Err(UploadError::MalformedImage)
        );
        assert_eq!(
            read_dimensions(&fixtures::jpeg(10, 0)),
            Err(UploadError::MalformedImage)
        );
    }

    #[test]
    fn rejects_scan_before_frame_header() {
        let data = [0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02];

        assert_eq!(read_dimensions(&data), Err(UploadError::MalformedImage));
    }

    #[test]
    fn rejects_segment_length_below_minimum() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x01];

        assert_eq!(read_dimensions(&data), Err(UploadError::MalformedImage));
    }

    #[test]
    fn rejects_segment_running_past_the_buffer() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0xFF, 0xFF, 0x00];

        assert_eq!(read_dimensions(&data), Err(UploadError::MalformedImage));
    }

    #[test]
    fn rejects_garbage_between_segments() {
        let data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0x12, 0x34];

        assert_eq!(read_dimensions(&data), Err(UploadError::MalformedImage));
    }

    #[test]
    fn skips_fill_bytes_before_markers() {
        let mut data = vec![0xFF, 0xD8, 0xFF, 0xFF, 0xFF];
        data.extend(&fixtures::jpeg(8, 9)[2..]);

        let dimensions = read_dimensions(&data).expect("fill bytes are legal");

        assert_eq!(
            dimensions,
            Dimensions {
                width: 8,
                height: 9
            }
        );
    }
}
