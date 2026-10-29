//! Shared helpers for the artwork-cache integration tests: per-test temp
//! dirs, generated images and the sample tree index fixture.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)] // test code: panics are the failure signal

use std::io::Cursor;
use std::path::Path;

use image::{ImageFormat, RgbImage};

/// Small hand-written GitHub tree response (see the fixture for its cases).
pub const SAMPLE_TREE_JSON: &[u8] = include_bytes!("../fixtures/snes-tree-sample.json");

/// Fresh, empty per-test directory under the system temp dir; removed on drop.
pub fn fresh_dir(test: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-artwork-cache-{test}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

/// A `width`×`height` gradient image encoded as `format`.
pub fn image_bytes(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
    let image = RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
    });
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).unwrap();
    bytes.into_inner()
}

/// File names directly inside `dir` (empty when it does not exist).
pub fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
