//! Synthetic PlayStation 2 volumes for tests: a `PLAYSTATION` ISO9660 volume whose
//! `SYSTEM.CNF` boots through `BOOT2`, with a chosen volume id, plus the ZSO (LZ4
//! block) container. Wrap the sectors with `test_images::{to_iso, to_raw, to_chd}`
//! and `test_psp_containers::{cso, dvd_chd}` for the other containers.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use crate::disc_sector_reader::SECTOR_DATA_BYTES;
use crate::test_images::iso_sectors;

/// Boot executable of the test disc, as `SYSTEM.CNF` names it (`;1` added there).
pub const TEST_EXE: &str = "NSTR_000.01";
/// Normalised serial of [`TEST_EXE`].
pub const TEST_SERIAL: &str = "NSTR-00001";
/// Volume id of [`ps2_sectors`] discs.
pub const TEST_VOLUME_ID: &str = "NOSTYRA_PS2";

/// A retail-shaped `SYSTEM.CNF` booting `cdrom0:\<exe>;1` in NTSC.
pub fn ps2_cnf(exe: &str) -> String {
    format!("BOOT2 = cdrom0:\\{exe};1\r\nVER = 1.00\r\nVMODE = NTSC\r\n")
}

/// The test PS2 disc: `SYSTEM.CNF` booting `exe` plus a stub executable of that name.
pub fn ps2_sectors(exe: &str) -> Vec<Vec<u8>> {
    let cnf = ps2_cnf(exe);
    ps2_volume(
        TEST_VOLUME_ID,
        &[("SYSTEM.CNF", cnf.as_bytes()), (exe, &elf_stub())],
    )
}

/// A `PLAYSTATION` volume named `volume_id` holding `files` in its root (no PS1
/// licence text).
pub fn ps2_volume(volume_id: &str, files: &[(&str, &[u8])]) -> Vec<Vec<u8>> {
    let mut sectors = iso_sectors("PLAYSTATION", None, files);
    sectors[16][40..72].copy_from_slice(format!("{volume_id:<32}").as_bytes());
    sectors
}

/// A few hundred bytes that look like an ELF (the probes never read it).
fn elf_stub() -> Vec<u8> {
    let mut elf = b"\x7fELF\x01\x01\x01".to_vec();
    elf.resize(512, 0);
    elf
}

/// ZSO of `iso` (CISO layout, `ZISO` magic): `block` bytes per block, offsets
/// shifted by `align`; blocks that LZ4-compress smaller are stored compressed,
/// others plain. Same shape as `test_psp_containers::cso`.
pub fn zso(iso: &[u8], block: usize, align: u8) -> Vec<u8> {
    let blocks = iso.len().div_ceil(block);
    let index_end = 0x18 + (blocks + 1) * 4;
    let pad = |out: &mut Vec<u8>| out.resize(out.len().next_multiple_of(1 << align), 0);
    let mut body = vec![0u8; index_end];
    pad(&mut body);
    let mut index = Vec::new();
    for chunk in iso.chunks(block) {
        let packed = lz4_flex::block::compress(chunk);
        let entry = (body.len() >> align) as u32;
        if packed.len() < chunk.len() {
            index.push(entry);
            body.extend(packed);
        } else {
            index.push(entry | 0x8000_0000);
            body.extend(chunk);
        }
        pad(&mut body);
    }
    index.push((body.len() >> align) as u32);
    body[..4].copy_from_slice(b"ZISO");
    body[4..8].copy_from_slice(&0x18u32.to_le_bytes());
    body[8..16].copy_from_slice(&(iso.len() as u64).to_le_bytes());
    body[16..20].copy_from_slice(&(block as u32).to_le_bytes());
    body[20] = 1;
    body[21] = align;
    for (i, entry) in index.iter().enumerate() {
        body[0x18 + i * 4..0x1C + i * 4].copy_from_slice(&entry.to_le_bytes());
    }
    debug_assert!(body.len() >= index_end && block >= SECTOR_DATA_BYTES);
    body
}
