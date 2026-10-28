use std::io::Cursor;

use eg_core::config::proof::ProofConfig;
use eg_image_worker::proof::{ProofError, ProofGenerator};
use image::{
    ExtendedColorType, ImageFormat, ImageReader, Rgb, RgbImage, codecs::jpeg::JpegEncoder,
};
use moxcms::ColorProfile;

const SECRET_FILENAME: &str = "IMG_SECRET_CLIENT_NAME.jpg";
const SECRET_COMMENT: &str = "PRIVATE-COMMENT-MARKER";

fn config() -> ProofConfig {
    ProofConfig {
        proof_max_edge: 2048,
        proof_quality: 80,
        proof_max_source_pixels: 50_000_000,
        proof_max_source_bytes: 128 * 1024 * 1024,
    }
}

/// Left half red, right half blue, so orientation changes are observable.
fn two_tone_image(width: u32, height: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            Rgb([255, 0, 0])
        } else {
            Rgb([0, 0, 255])
        }
    })
}

fn encode_jpeg(image: &RgbImage, quality: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgb8,
        )
        .expect("encode fixture");
    bytes
}

fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let length = u16::try_from(payload.len() + 2).expect("segment fits");
    let mut bytes = vec![0xFF, marker];
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

/// Minimal big-endian EXIF block: Orientation in IFD0 plus a GPS IFD holding
/// a latitude reference.
fn exif_with_gps(orientation: u16) -> Vec<u8> {
    let mut tiff = b"MM\x00\x2A\x00\x00\x00\x08".to_vec();
    tiff.extend_from_slice(&[0x00, 0x02]);
    tiff.extend_from_slice(&[0x01, 0x12, 0x00, 0x03, 0, 0, 0, 1]);
    tiff.extend_from_slice(&orientation.to_be_bytes());
    tiff.extend_from_slice(&[0, 0]);
    tiff.extend_from_slice(&[0x88, 0x25, 0x00, 0x04, 0, 0, 0, 1, 0, 0, 0, 26]);
    tiff.extend_from_slice(&[0, 0, 0, 0]);
    tiff.extend_from_slice(&[0x00, 0x01]);
    tiff.extend_from_slice(&[0x00, 0x01, 0x00, 0x02, 0, 0, 0, 2, b'N', 0, 0, 0]);
    tiff.extend_from_slice(&[0, 0, 0, 0]);

    let mut payload = b"Exif\0\0".to_vec();
    payload.extend_from_slice(&tiff);
    segment(0xE1, &payload)
}

/// Inserts segments right after the SOI marker.
fn with_segments(jpeg: &[u8], segments: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = jpeg[..2].to_vec();
    for segment in segments {
        bytes.extend_from_slice(segment);
    }
    bytes.extend_from_slice(&jpeg[2..]);
    bytes
}

fn tagged_source(image: &RgbImage, orientation: u16) -> Vec<u8> {
    let comment = segment(
        0xFE,
        format!("{SECRET_FILENAME} {SECRET_COMMENT}").as_bytes(),
    );
    with_segments(
        &encode_jpeg(image, 95),
        &[exif_with_gps(orientation), comment],
    )
}

/// Marker codes of the segments before the scan data.
fn header_markers(jpeg: &[u8]) -> Vec<u8> {
    let mut markers = Vec::new();
    let mut offset = 2;
    while offset + 4 <= jpeg.len() && jpeg[offset] == 0xFF {
        let marker = jpeg[offset + 1];
        markers.push(marker);
        if marker == 0xDA {
            break;
        }
        offset += 2 + usize::from(u16::from_be_bytes([jpeg[offset + 2], jpeg[offset + 3]]));
    }
    markers
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn decode(bytes: &[u8]) -> RgbImage {
    ImageReader::with_format(Cursor::new(bytes), ImageFormat::Jpeg)
        .decode()
        .expect("decode proof")
        .into_rgb8()
}

#[test]
fn large_source_is_capped_on_long_edge_keeping_aspect_ratio() {
    let generator = ProofGenerator::new(ProofConfig {
        proof_max_edge: 200,
        ..config()
    });

    let proof = generator
        .generate(&encode_jpeg(&two_tone_image(600, 300), 90))
        .expect("generate");

    assert_eq!((proof.width, proof.height), (200, 100));
    let decoded = decode(&proof.bytes);
    assert_eq!(decoded.dimensions(), (200, 100));
}

#[test]
fn small_source_is_never_upscaled() {
    let proof = ProofGenerator::new(config())
        .generate(&encode_jpeg(&two_tone_image(64, 48), 90))
        .expect("generate");

    assert_eq!((proof.width, proof.height), (64, 48));
}

#[test]
fn output_is_jpeg_and_quality_follows_config() {
    let source = encode_jpeg(
        &RgbImage::from_fn(300, 300, |x, y| {
            Rgb([
                (x * 7 % 256) as u8,
                (y * 13 % 256) as u8,
                (x * y % 256) as u8,
            ])
        }),
        98,
    );
    let at = |proof_quality| {
        ProofGenerator::new(ProofConfig {
            proof_quality,
            ..config()
        })
        .generate(&source)
        .expect("generate")
    };

    let (low, default, high) = (at(30), at(80), at(95));

    assert!(low.bytes.starts_with(&[0xFF, 0xD8, 0xFF]));
    assert_eq!(
        ImageReader::new(Cursor::new(&default.bytes))
            .with_guessed_format()
            .expect("guess")
            .format(),
        Some(ImageFormat::Jpeg)
    );
    assert!(low.bytes.len() < default.bytes.len());
    assert!(default.bytes.len() < high.bytes.len());
}

#[test]
fn exif_gps_comments_and_filenames_are_stripped() {
    let source = tagged_source(&two_tone_image(64, 48), 1);
    assert!(contains(&source, b"Exif"));
    assert!(contains(&source, SECRET_COMMENT.as_bytes()));

    let proof = ProofGenerator::new(config())
        .generate(&source)
        .expect("generate");

    for needle in [
        b"Exif".as_slice(),
        b"GPS",
        SECRET_FILENAME.as_bytes(),
        SECRET_COMMENT.as_bytes(),
    ] {
        assert!(!contains(&proof.bytes, needle), "leaked {needle:?}");
    }
    let markers = header_markers(&proof.bytes);
    assert!(
        !markers.iter().any(|m| matches!(m, 0xE1..=0xEF | 0xFE)),
        "unexpected metadata segments: {markers:02X?}"
    );
}

#[test]
fn exif_orientation_is_applied_before_stripping() {
    // Orientation 6: stored sideways, must be rotated 90 degrees clockwise.
    let source = tagged_source(&two_tone_image(40, 20), 6);

    let proof = ProofGenerator::new(config())
        .generate(&source)
        .expect("generate");

    assert_eq!((proof.width, proof.height), (20, 40));
    let decoded = decode(&proof.bytes);
    let top = decoded.get_pixel(10, 5);
    let bottom = decoded.get_pixel(10, 35);
    assert!(top[0] > 200 && top[2] < 60, "top should be red: {top:?}");
    assert!(
        bottom[2] > 200 && bottom[0] < 60,
        "bottom should be blue: {bottom:?}"
    );
}

#[test]
fn source_icc_profile_is_converted_to_srgb_then_dropped() {
    let image = RgbImage::from_pixel(32, 32, Rgb([200, 60, 60]));
    let icc = ColorProfile::new_adobe_rgb()
        .encode()
        .expect("encode profile");
    let mut payload = b"ICC_PROFILE\0\x01\x01".to_vec();
    payload.extend_from_slice(&icc);
    let source = with_segments(&encode_jpeg(&image, 95), &[segment(0xE2, &payload)]);
    let untagged = encode_jpeg(&image, 95);

    let converted = ProofGenerator::new(config())
        .generate(&source)
        .expect("generate");
    let plain = ProofGenerator::new(config())
        .generate(&untagged)
        .expect("generate");

    assert!(!contains(&converted.bytes, b"ICC_PROFILE"));
    let shifted = decode(&converted.bytes).get_pixel(16, 16)[0];
    let unshifted = decode(&plain.bytes).get_pixel(16, 16)[0];
    assert!(
        shifted > unshifted + 5,
        "AdobeRGB red should widen: {shifted} vs {unshifted}"
    );
}

#[test]
fn generation_is_deterministic() {
    let source = tagged_source(&two_tone_image(300, 200), 6);
    let generator = ProofGenerator::new(ProofConfig {
        proof_max_edge: 128,
        ..config()
    });

    assert_eq!(
        generator.generate(&source).expect("first"),
        generator.generate(&source).expect("second")
    );
}

#[test]
fn non_jpeg_input_is_rejected() {
    let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

    let error = ProofGenerator::new(config())
        .generate(png)
        .expect_err("png rejected");

    assert!(matches!(error, ProofError::NotJpeg));
}

#[test]
fn corrupt_input_is_rejected() {
    let jpeg = encode_jpeg(&two_tone_image(64, 64), 90);

    let truncated = ProofGenerator::new(config()).generate(&jpeg[..40]);
    let garbage = ProofGenerator::new(config()).generate(&[0xFF, 0xD8, 0xFF, 0x00, 0x01, 0x02]);

    assert!(matches!(truncated, Err(ProofError::Decode(_))));
    assert!(matches!(garbage, Err(ProofError::Decode(_))));
}

#[test]
fn oversized_dimensions_are_rejected_before_decoding() {
    let source = encode_jpeg(&two_tone_image(1000, 1000), 50);
    let generator = ProofGenerator::new(ProofConfig {
        proof_max_source_pixels: 500_000,
        ..config()
    });

    let error = generator.generate(&source).expect_err("too many pixels");

    assert!(matches!(
        error,
        ProofError::DimensionsTooLarge {
            width: 1000,
            height: 1000,
            ..
        }
    ));
}

#[test]
fn oversized_file_is_rejected() {
    let source = encode_jpeg(&two_tone_image(64, 64), 90);
    let generator = ProofGenerator::new(ProofConfig {
        proof_max_source_bytes: 100,
        ..config()
    });

    let error = generator.generate(&source).expect_err("too many bytes");

    assert!(matches!(
        error,
        ProofError::SourceTooLarge { limit: 100, .. }
    ));
}
