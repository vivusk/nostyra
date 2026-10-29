//! Validates downloaded art and produces the cached bytes. Covers are
//! decoded, fitted within 600×600 (libretro box art is ≤ 512 wide, so this
//! is normally a passthrough of the dimensions) and re-encoded as JPEG q85:
//! ~40–90 KB instead of ~270 KB PNG. Hero snaps (256×224 pixel art, tiny)
//! are only decoded to prove they are real PNGs and then stored unchanged.
//! Pure Rust (`image` with png/jpeg/webp only); decode is capped at
//! 4 096 px per side so a hostile file cannot allocate gigabytes.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Limits};

use crate::ArtworkError;

/// Longest side of a cached cover; bigger sources are scaled down to fit.
pub const COVER_MAX_SIDE: u32 = 600;
/// Largest width or height we agree to decode at all.
const MAX_DECODE_SIDE: u32 = 4096;
/// Visually lossless for box art at a fraction of the PNG size.
const COVER_JPEG_QUALITY: u8 = 85;

/// Decodes `bytes` (PNG, JPEG or WebP), fits it within
/// [`COVER_MAX_SIDE`]² keeping the aspect ratio and returns JPEG q85 bytes.
/// Transparency is dropped (box art is opaque).
pub fn encode_cover_jpeg(bytes: &[u8]) -> Result<Vec<u8>, ArtworkError> {
    let (image, _) = decode_capped(bytes)?;
    let image = if image.width().max(image.height()) > COVER_MAX_SIDE {
        image.resize(COVER_MAX_SIDE, COVER_MAX_SIDE, FilterType::Triangle)
    } else {
        image
    };
    let mut jpeg = Vec::new();
    image
        .to_rgb8()
        .write_with_encoder(JpegEncoder::new_with_quality(&mut jpeg, COVER_JPEG_QUALITY))
        .map_err(|error| ArtworkError::InvalidImage(format!("jpeg encode failed: {error}")))?;
    Ok(jpeg)
}

/// Accepts `bytes` only when they are a PNG that fully decodes within the
/// size cap; the caller then stores the original bytes untouched.
pub fn validate_hero_png(bytes: &[u8]) -> Result<(), ArtworkError> {
    let (_, format) = decode_capped(bytes)?;
    if format != ImageFormat::Png {
        return Err(ArtworkError::InvalidImage(format!(
            "expected PNG, got {format:?}"
        )));
    }
    Ok(())
}

/// Sniffs the format from the bytes (never from a URL or header) and decodes
/// with the dimension cap enforced before any pixel buffer is allocated.
fn decode_capped(bytes: &[u8]) -> Result<(DynamicImage, ImageFormat), ArtworkError> {
    let invalid = |error: image::ImageError| ArtworkError::InvalidImage(error.to_string());
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| ArtworkError::InvalidImage(error.to_string()))?;
    let format = reader
        .format()
        .ok_or_else(|| ArtworkError::InvalidImage("unrecognised image format".into()))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DECODE_SIDE);
    limits.max_image_height = Some(MAX_DECODE_SIDE);
    reader.limits(limits);
    let image = reader.decode().map_err(invalid)?;
    Ok((image, format))
}
