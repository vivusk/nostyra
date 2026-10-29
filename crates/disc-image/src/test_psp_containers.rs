//! Synthetic PSP containers for tests: `EBOOT.PBP`, CSO (CISO v1) and DVD CHD v5
//! images, plus the uncompressed CHD v5 writer shared with the CD/GD images.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::io::Write;

use flate2::write::DeflateEncoder;
use flate2::Compression;

use crate::disc_sector_reader::SECTOR_DATA_BYTES;

/// `EBOOT.PBP`: header, `sfo`, a 32-byte `DATA.PSP` stub, then `psar` as
/// `DATA.PSAR` (may be empty).
pub fn pbp(sfo: &[u8], psar: &[u8]) -> Vec<u8> {
    let sfo_end = 0x28 + sfo.len() as u32;
    let mut out = b"\0PBP".to_vec();
    out.extend(0x0001_0000u32.to_le_bytes());
    out.extend(0x28u32.to_le_bytes()); // PARAM.SFO
    for _ in 0..6 {
        out.extend(sfo_end.to_le_bytes()); // ICON0, ICON1, PIC0, PIC1, SND0 empty; DATA.PSP
    }
    out.extend((sfo_end + 32).to_le_bytes()); // DATA.PSAR
    out.extend(sfo);
    out.extend(b"~PSP");
    out.resize(out.len() + 28, 0);
    out.extend(psar);
    out
}

/// CISO v1 of `iso`: `block` bytes per block, offsets shifted by `align`; blocks that
/// deflate smaller are stored compressed, others plain (all-zero blocks deflate,
/// file bodies of random-looking bytes stay plain).
pub fn cso(iso: &[u8], block: usize, align: u8) -> Vec<u8> {
    let blocks = iso.len().div_ceil(block);
    let index_end = 0x18 + (blocks + 1) * 4;
    let pad = |out: &mut Vec<u8>| out.resize(out.len().next_multiple_of(1 << align), 0);
    let mut body = vec![0u8; index_end];
    pad(&mut body);
    let mut index = Vec::new();
    for chunk in iso.chunks(block) {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(chunk).unwrap();
        let packed = encoder.finish().unwrap();
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
    body[..4].copy_from_slice(b"CISO");
    body[4..8].copy_from_slice(&0x18u32.to_le_bytes());
    body[8..16].copy_from_slice(&(iso.len() as u64).to_le_bytes());
    body[16..20].copy_from_slice(&(block as u32).to_le_bytes());
    body[20] = 1;
    body[21] = align;
    for (i, entry) in index.iter().enumerate() {
        body[0x18 + i * 4..0x1C + i * 4].copy_from_slice(&entry.to_le_bytes());
    }
    body
}

/// DVD CHD v5 of 2048-byte sectors as chdman writes them: unit 2048, hunk 4096,
/// optionally a `DVD ` metadata entry.
pub fn dvd_chd(iso: &[u8], dvd_tag: bool) -> Vec<u8> {
    let metas = if dvd_tag {
        vec![(*b"DVD ", String::new())]
    } else {
        Vec::new()
    };
    chd_v5_units(iso, SECTOR_DATA_BYTES, 4096, iso.len() as u64, &metas)
}

/// Uncompressed CHD v5: `data` (`unit`-byte units) packed into `hunk`-byte hunks
/// after a header + map block, `logical` bytes declared, metadata (tag, text)
/// chained in order after the hunks.
pub fn chd_v5_units(
    data: &[u8],
    unit: usize,
    hunk: usize,
    logical: u64,
    metas: &[([u8; 4], String)],
) -> Vec<u8> {
    let hunks = data.len().div_ceil(hunk);
    let header = (124 + hunks * 4).next_multiple_of(hunk);
    let mut out = vec![0u8; header];
    out[..8].copy_from_slice(b"MComprHD");
    out[8..12].copy_from_slice(&124u32.to_be_bytes());
    out[12..16].copy_from_slice(&5u32.to_be_bytes());
    out[32..40].copy_from_slice(&logical.to_be_bytes());
    out[40..48].copy_from_slice(&124u64.to_be_bytes());
    let meta_offset = if metas.is_empty() {
        0
    } else {
        header + hunk * hunks
    };
    out[48..56].copy_from_slice(&(meta_offset as u64).to_be_bytes());
    out[56..60].copy_from_slice(&(hunk as u32).to_be_bytes());
    out[60..64].copy_from_slice(&(unit as u32).to_be_bytes());
    for h in 0..hunks {
        let at = 124 + h * 4;
        out[at..at + 4].copy_from_slice(&((header / hunk + h) as u32).to_be_bytes());
    }
    out.extend(data);
    out.resize(header + hunks * hunk, 0);
    for (i, (tag, text)) in metas.iter().enumerate() {
        let value = format!("{text}\0");
        let next = if i + 1 < metas.len() {
            (out.len() + 16 + value.len()) as u64
        } else {
            0
        };
        out.extend(tag);
        out.extend((value.len() as u32).to_be_bytes());
        out.extend(next.to_be_bytes());
        out.extend(value.as_bytes());
    }
    out
}
