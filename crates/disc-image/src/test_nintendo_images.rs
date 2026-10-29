//! Synthetic GameCube / Wii fixtures for tests here and in dependent crates: a
//! 0x80-byte disc header (ID, disc, revision, magic, title — no Nintendo bytes)
//! wrapped in each container the disc reader knows, and tiny PowerPC `.dol` /
//! `.elf` files. Container layouts mirror the structs cited in
//! `nintendo_disc_containers.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::io::Write;

use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::nintendo_disc_header::{NintendoDiscKind, DISC_HEADER_BYTES};

/// `mtspr HID4, r0`: the Wii-only instruction the GC/Wii split looks for.
pub const MTSPR_HID4_R0: u32 = 0x7C13_FBA6;
/// `blr` (any harmless PowerPC word).
pub const BLR: u32 = 0x4E80_0020;
/// Load address of the first text section of [`dol`] / segment of [`elf`].
pub const CODE_ADDR: u32 = 0x8000_3100;
/// Block size of the [`ciso`] and [`gcz`] fixtures.
pub const FIXTURE_BLOCK: usize = 0x4000;

/// Disc header of `kind` with game ID `id` (6 chars), disc and revision bytes.
pub fn disc_header(kind: NintendoDiscKind, id: &str, disc: u8, rev: u8) -> [u8; DISC_HEADER_BYTES] {
    let mut h = [0u8; DISC_HEADER_BYTES];
    h[..6].copy_from_slice(id.as_bytes());
    h[6] = disc;
    h[7] = rev;
    match kind {
        NintendoDiscKind::Wii => h[0x18..0x1C].copy_from_slice(&0x5D1C_9EA3u32.to_be_bytes()),
        NintendoDiscKind::GameCube => h[0x1C..0x20].copy_from_slice(&0xC233_9F3Du32.to_be_bytes()),
    }
    h[0x20..0x2D].copy_from_slice(b"NOSTYRA TEST\0");
    h
}

/// Plain `.iso` / `.gcm`: the header, then zeros up to `len` bytes.
pub fn plain(header: &[u8], len: usize) -> Vec<u8> {
    let mut out = header.to_vec();
    out.resize(len.max(header.len()), 0);
    out
}

/// `.ciso`: magic, block size, block map (block 0 used when `block0_used`), then
/// block 0 = the plain disc start.
pub fn ciso(header: &[u8], block0_used: bool) -> Vec<u8> {
    let mut out = b"CISO".to_vec();
    out.extend((FIXTURE_BLOCK as u32).to_le_bytes());
    out.resize(0x8000, 0);
    out[8] = u8::from(block0_used);
    out.extend(plain(header, FIXTURE_BLOCK));
    out
}

/// `.wia` / `.rvz`: `WIAHeader1` (0x48 bytes), `WIAHeader2` (4 × u32, then the raw
/// disc header), padded to 0x200.
pub fn wia(header: &[u8], rvz: bool) -> Vec<u8> {
    let mut out = if rvz { b"RVZ\x01" } else { b"WIA\x01" }.to_vec();
    out.extend(0x0100_0000u32.to_be_bytes()); // version
    out.resize(0x48, 0);
    out.extend(2u32.to_be_bytes()); // disc_type (Wii); the reader ignores it
    out.extend([0u8; 12]); // compression, level, chunk size
    out.extend(header);
    out.resize(0x200, 0);
    out
}

/// `.wbfs` with 2^`shift`-byte HD sectors: header (disc slot 0 used), then the disc
/// header at sector 1.
pub fn wbfs(header: &[u8], shift: u8) -> Vec<u8> {
    let mut out = b"WBFS".to_vec();
    out.extend(16u32.to_be_bytes()); // HD sector count
    out.extend([shift, 21, 0, 0, 1]); // hd / wbfs sector shift, pad, disc table[0]
    out.resize(1 << shift, 0);
    out.extend(header);
    out.resize(out.len() + 0x100, 0);
    out
}

/// `.gcz` of a `blocks`-block disc starting with `header`: block 0 stored plain
/// when `stored`, every other block zlib-compressed.
pub fn gcz(header: &[u8], blocks: usize, stored: bool) -> Vec<u8> {
    let disc = plain(header, FIXTURE_BLOCK * blocks);
    let (mut offsets, mut data) = (Vec::new(), Vec::new());
    for (i, chunk) in disc.chunks(FIXTURE_BLOCK).enumerate() {
        if i == 0 && stored {
            offsets.push(data.len() as u64 | 1 << 63);
            data.extend(chunk);
        } else {
            offsets.push(data.len() as u64);
            let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
            encoder.write_all(chunk).unwrap();
            data.extend(encoder.finish().unwrap());
        }
    }
    let mut out = 0xB10B_C001u32.to_le_bytes().to_vec();
    out.extend(0u32.to_le_bytes()); // sub type
    out.extend((data.len() as u64).to_le_bytes());
    out.extend((disc.len() as u64).to_le_bytes());
    out.extend((FIXTURE_BLOCK as u32).to_le_bytes());
    out.extend((blocks as u32).to_le_bytes());
    offsets.iter().for_each(|o| out.extend(o.to_le_bytes()));
    out.extend(vec![0u8; 4 * blocks]); // adler32 per block (not checked)
    out.extend(data);
    out
}

/// `.dol`: one text section of `text` words @[`CODE_ADDR`] (entry = its start) and,
/// when `data` is not empty, one data section right after it.
pub fn dol(text: &[u32], data: &[u32]) -> Vec<u8> {
    let mut out = vec![0u8; 0x100];
    let put = |out: &mut Vec<u8>, at: usize, value: u32| {
        out[at..at + 4].copy_from_slice(&value.to_be_bytes());
    };
    let (text_len, data_len) = (4 * text.len() as u32, 4 * data.len() as u32);
    put(&mut out, 0x00, 0x100); // text 0 file offset
    put(&mut out, 0x48, CODE_ADDR);
    put(&mut out, 0x90, text_len);
    if !data.is_empty() {
        put(&mut out, 0x1C, 0x100 + text_len); // data 0 file offset
        put(&mut out, 0x64, CODE_ADDR + text_len);
        put(&mut out, 0xAC, data_len);
    }
    put(&mut out, 0xE0, CODE_ADDR);
    text.iter()
        .chain(data)
        .for_each(|w| out.extend(w.to_be_bytes()));
    out
}

/// ELF32 big-endian for `machine` (20 = PowerPC): one `PT_LOAD` R+X segment of
/// `code` words @0x100, loaded at [`CODE_ADDR`].
pub fn elf(machine: u16, code: &[u32]) -> Vec<u8> {
    let mut out = b"\x7FELF\x01\x02\x01".to_vec();
    out.resize(16, 0);
    out.extend(2u16.to_be_bytes()); // ET_EXEC
    out.extend(machine.to_be_bytes());
    out.extend(1u32.to_be_bytes()); // version
    out.extend(CODE_ADDR.to_be_bytes()); // entry
    out.extend(52u32.to_be_bytes()); // e_phoff
    out.extend(0u32.to_be_bytes()); // e_shoff
    out.extend(0u32.to_be_bytes()); // e_flags
    for half in [52u16, 32, 1, 40, 0, 0] {
        out.extend(half.to_be_bytes()); // ehsize, phentsize, phnum, shentsize, shnum, shstrndx
    }
    let size = 4 * code.len() as u32;
    for word in [1, 0x100, CODE_ADDR, CODE_ADDR, size, size, 5, 4] {
        out.extend(word.to_be_bytes()); // PT_LOAD, offset, vaddr, paddr, filesz, memsz, R+X, align
    }
    out.resize(0x100, 0);
    code.iter().for_each(|w| out.extend(w.to_be_bytes()));
    out
}
