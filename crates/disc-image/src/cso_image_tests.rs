//! CSO reader tests: in-code CISO v1 images (deflate + plain blocks, align, bigger
//! blocks with a partial last one) read back sector by sector; broken headers/index.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use super::*;
use crate::test_images::write;
use crate::test_psp_containers::cso;

/// 7 sectors: zeros (deflate well) and pseudo-random bytes (stored plain).
fn sample_iso() -> Vec<u8> {
    let mut iso = vec![0u8; SECTOR_DATA_BYTES * 7];
    let mut x = 0x1234_5678u32;
    for byte in &mut iso[SECTOR_DATA_BYTES * 2..SECTOR_DATA_BYTES * 4] {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        *byte = x as u8;
    }
    for (lba, sector) in iso.chunks_mut(SECTOR_DATA_BYTES).enumerate() {
        sector[0] = lba as u8 + 1;
    }
    iso
}

fn read_all(path: &Path, sectors: usize) -> Vec<u8> {
    let mut reader = CsoReader::open(path).unwrap();
    let mut out = Vec::new();
    for lba in (0..sectors as u32).rev() {
        out.splice(0..0, reader.read_user_data(lba).unwrap());
    }
    assert!(matches!(
        reader.read_user_data(sectors as u32),
        Err(DiscError::SectorOutOfRange(_))
    ));
    out
}

#[test]
fn deflated_and_plain_blocks_read_back_with_and_without_align() {
    let dir = tempfile::tempdir().unwrap();
    let iso = sample_iso();
    for align in [0, 2] {
        let image = cso(&iso, SECTOR_DATA_BYTES, align);
        let path = write(dir.path(), &format!("a{align}.cso"), &image);
        assert!(image.len() < iso.len(), "zeros compressed");
        assert_eq!(read_all(&path, 7), iso);
    }
}

#[test]
fn bigger_blocks_with_a_partial_last_block() {
    let dir = tempfile::tempdir().unwrap();
    let iso = sample_iso(); // 7 sectors = 3.5 blocks of 4 KiB
    let path = write(dir.path(), "b.cso", &cso(&iso, 4096, 0));
    assert_eq!(read_all(&path, 7), iso);
}

#[test]
fn unsupported_and_broken_files_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let good = cso(&sample_iso(), SECTOR_DATA_BYTES, 0);
    let open = |name: &str, bytes: &[u8]| CsoReader::open(&write(dir.path(), name, bytes));
    let error = |result: Result<CsoReader, DiscError>| result.err().unwrap().to_string();
    let mut zso = good.clone();
    zso[..4].copy_from_slice(b"ZISO");
    assert_eq!(
        error(open("z.cso", &zso)),
        "ZSO is not supported by the PSP core"
    );
    let mut v2 = good.clone();
    v2[20] = 2;
    assert_eq!(error(open("v2.cso", &v2)), "CSO version 2 is not supported");
    let mut odd_block = good.clone();
    odd_block[16..20].copy_from_slice(&3000u32.to_le_bytes());
    assert!(open("odd.cso", &odd_block).is_err());
    assert!(open("junk.cso", b"not a cso at all, no").is_err());
    assert!(open("short.cso", b"CISO").is_err());
    let mut corrupt = good;
    corrupt[0x18 + 4..0x18 + 8].copy_from_slice(&0u32.to_le_bytes()); // block 1 ends before it starts
    let mut reader = open("bad-index.cso", &corrupt).unwrap();
    assert!(reader.read_user_data(0).is_err());
}
