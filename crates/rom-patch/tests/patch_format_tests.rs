//! IPS / UPS / BPS application, checksum validation, copier-header retry, lookup.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use rom_patch::{
    apply_patch, apply_patch_chain, find_patches_for_rom, patch_chain_crc32, PatchError,
    PatchFormat,
};

fn rom() -> Vec<u8> {
    (0..2048u32).map(|i| (i * 7 % 251) as u8).collect()
}

/// byuu varint encoder (inverse of the reader).
fn varint(mut value: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let x = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(x | 0x80);
            return out;
        }
        out.push(x);
        value -= 1;
    }
}

/// Appends source/target CRCs and the patch's own CRC.
fn with_footer(mut patch: Vec<u8>, source: &[u8], target: &[u8]) -> Vec<u8> {
    patch.extend_from_slice(&crc32fast::hash(source).to_le_bytes());
    patch.extend_from_slice(&crc32fast::hash(target).to_le_bytes());
    let own = crc32fast::hash(&patch);
    patch.extend_from_slice(&own.to_le_bytes());
    patch
}

#[test]
fn ips_records_rle_growth_and_truncation() {
    let source = rom();
    let mut patch = b"PATCH".to_vec();
    patch.extend_from_slice(&[0x00, 0x00, 0x10, 0x00, 0x03, 0xAA, 0xBB, 0xCC]); // data
    patch.extend_from_slice(&[0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x04, 0x11]); // RLE ×4
    patch.extend_from_slice(&[0x00, 0x08, 0x00, 0x00, 0x02, 0x01, 0x02]); // past the end
    patch.extend_from_slice(b"EOF");
    let (format, out) = apply_patch(&source, &patch).unwrap();
    assert_eq!(format, PatchFormat::Ips);
    assert_eq!(&out[0x10..0x13], &[0xAA, 0xBB, 0xCC]);
    assert_eq!(&out[0x20..0x24], &[0x11; 4]);
    assert_eq!(out.len(), 0x802);
    assert_eq!(&out[0x800..], &[0x01, 0x02]);
    assert_eq!(out[0x13], source[0x13], "untouched bytes kept");

    let mut truncating = b"PATCH".to_vec();
    truncating.extend_from_slice(b"EOF");
    truncating.extend_from_slice(&[0x00, 0x04, 0x00]);
    assert_eq!(apply_patch(&source, &truncating).unwrap().1.len(), 0x400);
    assert_eq!(
        apply_patch(&source, b"PATCH\x00\x00"),
        Err(PatchError::Truncated)
    );
}

/// UPS changing bytes 5 and 6 and growing the image by 2 bytes.
fn ups_patch(source: &[u8], target: &[u8]) -> Vec<u8> {
    let mut patch = b"UPS1".to_vec();
    patch.extend(varint(source.len()));
    patch.extend(varint(target.len()));
    patch.extend(varint(5)); // skip 5
    patch.extend([source[5] ^ target[5], source[6] ^ target[6], 0x00]);
    let tail = source.len() - 8; // after the terminator we sit at 8
    patch.extend(varint(tail));
    patch.extend([target[source.len()], target[source.len() + 1], 0x00]);
    with_footer(patch, source, target)
}

#[test]
fn ups_applies_and_checks_checksums() {
    let source = rom();
    let mut target = source.clone();
    target[5] = 0xF0;
    target[6] = 0x0F;
    target.extend([0x33, 0x44]);
    let patch = ups_patch(&source, &target);
    assert_eq!(
        apply_patch(&source, &patch).unwrap(),
        (PatchFormat::Ups, target)
    );

    let mut other = source.clone();
    other[100] ^= 1;
    assert_eq!(apply_patch(&other, &patch), Err(PatchError::SourceMismatch));
    let mut corrupt = patch.clone();
    corrupt[6] ^= 0xFF;
    assert_eq!(
        apply_patch(&source, &corrupt),
        Err(PatchError::PatchChecksum)
    );
}

/// BPS using every action: SourceRead 4, TargetRead 2, SourceCopy 3 from 100,
/// TargetCopy 4 from 0, then SourceRead the rest.
fn bps_patch(source: &[u8], target: &[u8]) -> Vec<u8> {
    let mut patch = b"BPS1".to_vec();
    patch.extend(varint(source.len()));
    patch.extend(varint(target.len()));
    patch.extend(varint(0));
    let action = |cmd: usize, len: usize| varint(((len - 1) << 2) | cmd);
    patch.extend(action(0, 4));
    patch.extend(action(1, 2));
    patch.extend([0xDE, 0xAD]);
    patch.extend(action(2, 3));
    patch.extend(varint(100 << 1)); // +100
    patch.extend(action(3, 4));
    patch.extend(varint(0)); // from target offset 0
    patch.extend(action(0, source.len() - 13));
    with_footer(patch, source, target)
}

#[test]
fn bps_applies_all_actions_and_checks_checksums() {
    let source = rom();
    let mut target = source[..4].to_vec();
    target.extend([0xDE, 0xAD]);
    target.extend_from_slice(&source[100..103]);
    target.extend_from_slice(&source[..4]);
    target.extend_from_slice(&source[13..]);
    let patch = bps_patch(&source, &target);
    assert_eq!(
        apply_patch(&source, &patch).unwrap(),
        (PatchFormat::Bps, target)
    );

    let mut other = source.clone();
    other[0] ^= 1;
    assert_eq!(apply_patch(&other, &patch), Err(PatchError::SourceMismatch));
}

#[test]
fn headerless_patch_applies_to_a_headered_dump() {
    let source = rom();
    let mut target = source.clone();
    target[5] = 0xF0;
    target[6] = 0x0F;
    target.extend([0x33, 0x44]);
    let patch = ups_patch(&source, &target);
    let mut headered = vec![0u8; 512];
    headered.extend_from_slice(&source);
    assert_eq!(apply_patch(&headered, &patch).unwrap().1, target);
}

#[test]
fn unknown_format_is_rejected() {
    assert_eq!(apply_patch(&rom(), b"NOPE"), Err(PatchError::UnknownFormat));
}

/// IPS writing `bytes` at `offset`.
fn ips_patch(offset: u32, bytes: &[u8]) -> Vec<u8> {
    let mut patch = b"PATCH".to_vec();
    patch.extend_from_slice(&offset.to_be_bytes()[1..]);
    patch.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    patch.extend_from_slice(bytes);
    patch.extend_from_slice(b"EOF");
    patch
}

#[test]
fn patch_lookup_chains_formats_then_numbers_with_the_same_name() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-patch-")
        .tempdir()
        .unwrap();
    let dir = temp.path();
    let rom_path = dir.join("Game (USA).sfc");
    std::fs::write(&rom_path, b"rom").unwrap();
    assert!(find_patches_for_rom(&rom_path).is_empty());
    for name in [
        "Game (USA).ips",
        "Game (USA).ips2",
        "Game (USA).ips1",
        "Game (USA).ips4", // after the ips3 gap: ignored
        "Game (USA).ups1", // no plain .ups: numbering still counts
        "Game (USA).bps",
        "Other.bps",
    ] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    let names: Vec<String> = find_patches_for_rom(&rom_path)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "Game (USA).bps",
            "Game (USA).ups1",
            "Game (USA).ips",
            "Game (USA).ips1",
            "Game (USA).ips2"
        ]
    );
}

#[test]
fn chain_applies_in_order_and_reports_the_failing_patch() {
    let source = rom();
    let first = ips_patch(0x10, &[0xAA, 0xBB]);
    let second = ips_patch(0x11, &[0xCC]); // overwrites part of the first
    let (formats, out) = apply_patch_chain(&source, &[&first, &second]).unwrap();
    assert_eq!(formats, [PatchFormat::Ips, PatchFormat::Ips]);
    assert_eq!(&out[0x10..0x12], &[0xAA, 0xCC]);

    let broken: &[u8] = b"NOPE";
    assert_eq!(
        apply_patch_chain(&source, &[first.as_slice(), broken]),
        Err((1, PatchError::UnknownFormat))
    );
    let none: [&[u8]; 0] = [];
    assert_eq!(apply_patch_chain(&source, &none).unwrap().1, source);
}

#[test]
fn chain_crc_is_the_file_crc_for_one_patch() {
    let none: [&[u8]; 0] = [];
    assert_eq!(patch_chain_crc32(&none), None);
    assert_eq!(
        patch_chain_crc32(&[b"PATCH"]),
        Some(crc32fast::hash(b"PATCH"))
    );
    assert_eq!(
        patch_chain_crc32(&[b"PA".as_slice(), b"TCH".as_slice()]),
        Some(crc32fast::hash(b"PATCH"))
    );
}
