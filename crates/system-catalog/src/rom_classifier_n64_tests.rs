//! Classifier tests for the Nintendo 64: any of the three byte-order magics decides,
//! whatever the extension; anything else is reported, never guessed.

use super::*;
use crate::{n64_to_order, N64ByteOrder, NoDiscProbe};

fn head(ext: &str, bytes: &[u8], file_len: u64) -> Result<&'static SystemSpec, ClassifyError> {
    classify_with_head(
        &ALL_SYSTEMS,
        ext,
        file_len,
        Some(&bytes[..bytes.len().min(PROBE_BYTES)]),
        Path::new("mem"),
        &NoDiscProbe,
    )
}

/// 16 KiB big-endian (`.z64`) image.
fn z64_rom() -> Vec<u8> {
    let mut rom: Vec<u8> = (0..0x4000u32).map(|i| (i * 17 % 251) as u8).collect();
    rom[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
    rom
}

#[test]
fn every_magic_under_every_extension_is_n64() {
    let z64 = z64_rom();
    for order in [
        N64ByteOrder::BigEndian,
        N64ByteOrder::ByteSwapped,
        N64ByteOrder::LittleEndian,
    ] {
        let dump = n64_to_order(&z64, order);
        for ext in ["z64", "V64", "n64"] {
            assert_eq!(
                head(ext, &dump, dump.len() as u64).map(|s| s.id),
                Ok("n64"),
                "{order:?} as .{ext}"
            );
        }
    }
}

#[test]
fn junk_and_short_files_are_reported() {
    let junk = vec![0x55u8; 0x4000];
    assert_eq!(
        head("z64", &junk, junk.len() as u64),
        Err(ClassifyError::ProbeFailed("missing N64 header"))
    );
    // A head shorter than the magic (size check passed) fails the probe.
    assert_eq!(
        head("v64", &[0x37, 0x80, 0x40], 0x4000),
        Err(ClassifyError::ProbeFailed("missing N64 header"))
    );
    // Below 4 KiB the size check already refuses it.
    assert_eq!(
        head("n64", &z64_rom()[..3], 3),
        Err(ClassifyError::OutOfSize)
    );
}
