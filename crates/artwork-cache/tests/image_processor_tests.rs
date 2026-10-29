//! Cover re-encode and hero validation on generated images.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use artwork_cache::{encode_cover_jpeg, validate_hero_png};
use image::{GenericImageView, ImageFormat};
use support::image_bytes;

/// Re-encodes a `width`×`height` PNG and returns the JPEG's dimensions.
fn cover_dims(width: u32, height: u32) -> (u32, u32) {
    let jpeg = encode_cover_jpeg(&image_bytes(width, height, ImageFormat::Png)).unwrap();
    assert_eq!(image::guess_format(&jpeg).unwrap(), ImageFormat::Jpeg);
    image::load_from_memory(&jpeg).unwrap().dimensions()
}

#[test]
fn landscape_boxart_keeps_its_size() {
    assert_eq!(cover_dims(512, 364), (512, 364));
}

#[test]
fn square_art_under_the_cap_is_unchanged() {
    assert_eq!(cover_dims(300, 300), (300, 300));
}

#[test]
fn large_art_is_fitted_within_600() {
    assert_eq!(cover_dims(1200, 800), (600, 400));
    assert_eq!(cover_dims(700, 1400), (300, 600));
}

#[test]
fn jpeg_and_webp_sources_are_accepted_for_covers() {
    assert!(encode_cover_jpeg(&image_bytes(64, 48, ImageFormat::Jpeg)).is_ok());
    assert!(encode_cover_jpeg(&image_bytes(64, 48, ImageFormat::WebP)).is_ok());
}

#[test]
fn oversized_dimensions_are_rejected() {
    let huge = image_bytes(4097, 8, ImageFormat::Png);
    assert!(encode_cover_jpeg(&huge).is_err());
    assert!(validate_hero_png(&huge).is_err());
}

#[test]
fn garbage_is_rejected_without_panic() {
    assert!(encode_cover_jpeg(b"definitely not an image").is_err());
    assert!(encode_cover_jpeg(&[]).is_err());
    let mut truncated = image_bytes(256, 224, ImageFormat::Png);
    truncated.truncate(truncated.len() / 2);
    assert!(validate_hero_png(&truncated).is_err());
}

#[test]
fn hero_must_be_a_png() {
    assert!(validate_hero_png(&image_bytes(256, 224, ImageFormat::Png)).is_ok());
    assert!(validate_hero_png(&image_bytes(256, 224, ImageFormat::Jpeg)).is_err());
}
