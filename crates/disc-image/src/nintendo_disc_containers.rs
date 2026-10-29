//! Where each GameCube / Wii container keeps the raw 0x80-byte disc header (Dolphin
//! DiscIO, libretro/dolphin @ 3aac3f05). Only that header is read, plus the
//! container's own fixed fields; every offset is checked against the file length
//! before any seek, and nothing is allocated from header-supplied sizes.
//!
//! | ext | container | header location |
//! |---|---|---|
//! | `.iso` `.gcm` | plain | 0 |
//! | `.ciso` | `"CISO"`, block size u32 LE, 0x7FF8-byte block map (`CISOBlob.h`) | 0x8000 when block 0 is used (map[0] = 1) |
//! | `.wia` `.rvz` | `"WIA\x01"` / `"RVZ\x01"`: `WIAHeader1` is 0x48 bytes, `WIAHeader2` = disc_type, compression, compression_level, chunk_size (4 × u32), then `disc_header[0x80]` raw (`WIABlob.h` L81–101) | 0x48 + 0x10 = 0x58 |
//! | `.wbfs` | `"WBFS"`, sector count u32 BE, `hd_sector_shift` u8, `wbfs_sector_shift` u8, 2 pad, disc table @0x0C (`WbfsBlob.cpp` L95–119) | `1 << hd_sector_shift` (disc slot 0) |
//! | `.gcz` | magic `0xB10BC001` LE, sub type, compressed size u64, disc size u64, block size u32, block count u32 (32 bytes), then u64 block offsets (bit 63 = stored plain), u32 hashes, data (`CompressedBlob.h`) | start of block 0, zlib-inflated unless stored |
//!
//! A PSP `.cso` also starts with `CISO`: the extension, not the magic, picks the
//! reader, and this module never sees `.cso` files.

use std::io::{Read, Seek, SeekFrom};

use flate2::read::ZlibDecoder;

use crate::nintendo_disc_header::{DISC_HEADER_BYTES, MAX_HEADER_READ_BYTES};
use crate::{DiscError, CISO_EXT, GCZ_EXT, RVZ_EXT, WBFS_EXT, WIA_EXT};

type Head = [u8; DISC_HEADER_BYTES];

const CISO_HEADER_BYTES: u64 = 0x8000;
/// `sizeof(WIAHeader1)` + the four u32 fields of `WIAHeader2` before `disc_header`.
const WIA_DISC_HEADER_AT: usize = 0x48 + 0x10;
const WIA_MAGICS: [&[u8; 4]; 2] = [b"WIA\x01", b"RVZ\x01"];
/// 512-byte .. 32 KiB WBFS sectors keep the header read inside the budget.
const WBFS_SHIFTS: std::ops::RangeInclusive<u8> = 9..=15;
const GCZ_MAGIC: u32 = 0xB10B_C001;
const GCZ_HEADER_BYTES: u64 = 32;
const GCZ_STORED_BIT: u64 = 1 << 63;
/// Compressed block 0 may use what the budget leaves after the header + 2 offsets.
const MAX_GCZ_BLOCK_READ: u64 = MAX_HEADER_READ_BYTES - (GCZ_HEADER_BYTES + 16);

/// The raw disc header inside the container `ext` names; `None` when the file is
/// not that container (wrong magic, too short) or holds no header (CISO block 0
/// unused). [`DiscError::Format`] = the container is recognised but damaged.
pub(crate) fn read_container_head<R: Read + Seek>(
    reader: &mut R,
    ext: &str,
    file_len: u64,
) -> Result<Option<Head>, DiscError> {
    let is = |known: &str| ext.eq_ignore_ascii_case(known);
    if is(CISO_EXT) {
        ciso_head(reader, file_len)
    } else if is(WIA_EXT) || is(RVZ_EXT) {
        wia_head(reader, file_len)
    } else if is(WBFS_EXT) {
        wbfs_head(reader, file_len)
    } else if is(GCZ_EXT) {
        gcz_head(reader, file_len)
    } else {
        let mut head = [0u8; DISC_HEADER_BYTES];
        Ok(read_at(reader, 0, &mut head, file_len)?.then_some(head))
    }
}

/// Seeks to `at` and fills `buf`; `false` (no I/O) when that range passes the end.
fn read_at<R: Read + Seek>(
    reader: &mut R,
    at: u64,
    buf: &mut [u8],
    file_len: u64,
) -> Result<bool, DiscError> {
    let fits = at
        .checked_add(buf.len() as u64)
        .is_some_and(|end| end <= file_len);
    if fits {
        reader.seek(SeekFrom::Start(at))?;
        reader.read_exact(buf)?;
    }
    Ok(fits)
}

fn ciso_head<R: Read + Seek>(reader: &mut R, file_len: u64) -> Result<Option<Head>, DiscError> {
    let mut start = [0u8; 9]; // magic, block size, map[0]
    if !read_at(reader, 0, &mut start, file_len)? || &start[..4] != b"CISO" {
        return Ok(None);
    }
    let block_bytes = u32::from_le_bytes([start[4], start[5], start[6], start[7]]);
    if block_bytes < DISC_HEADER_BYTES as u32 {
        return Err(DiscError::Format("damaged CISO header"));
    }
    if start[8] != 1 {
        return Ok(None);
    }
    let mut head = [0u8; DISC_HEADER_BYTES];
    match read_at(reader, CISO_HEADER_BYTES, &mut head, file_len)? {
        true => Ok(Some(head)),
        false => Err(DiscError::Format("CISO image is truncated")),
    }
}

fn wia_head<R: Read + Seek>(reader: &mut R, file_len: u64) -> Result<Option<Head>, DiscError> {
    let mut start = [0u8; WIA_DISC_HEADER_AT + DISC_HEADER_BYTES];
    if !read_at(reader, 0, &mut start, file_len)? || !WIA_MAGICS.iter().any(|m| start[..4] == m[..])
    {
        return Ok(None);
    }
    let mut head = [0u8; DISC_HEADER_BYTES];
    head.copy_from_slice(&start[WIA_DISC_HEADER_AT..]);
    Ok(Some(head))
}

fn wbfs_head<R: Read + Seek>(reader: &mut R, file_len: u64) -> Result<Option<Head>, DiscError> {
    let mut start = [0u8; 13]; // magic, sector count, 2 shifts, pad, disc table[0]
    if !read_at(reader, 0, &mut start, file_len)? || &start[..4] != b"WBFS" {
        return Ok(None);
    }
    let shift = start[8];
    if !WBFS_SHIFTS.contains(&shift) {
        return Err(DiscError::Format("damaged WBFS header"));
    }
    if start[12] == 0 {
        return Err(DiscError::Format("WBFS image holds no disc"));
    }
    let mut head = [0u8; DISC_HEADER_BYTES];
    match read_at(reader, 1u64 << shift, &mut head, file_len)? {
        true => Ok(Some(head)),
        false => Err(DiscError::Format("WBFS image is truncated")),
    }
}

fn gcz_head<R: Read + Seek>(reader: &mut R, file_len: u64) -> Result<Option<Head>, DiscError> {
    const DAMAGED: DiscError = DiscError::Format("damaged GCZ header");
    let mut start = [0u8; GCZ_HEADER_BYTES as usize + 8]; // header + offsets[0]
    let le64 = |b: &[u8]| u64::from_le_bytes(b.try_into().unwrap_or_default());
    if !read_at(reader, 0, &mut start, file_len)?
        || u32::from_le_bytes([start[0], start[1], start[2], start[3]]) != GCZ_MAGIC
    {
        return Ok(None);
    }
    let block_bytes = u32::from_le_bytes([start[24], start[25], start[26], start[27]]);
    let blocks = u64::from(u32::from_le_bytes([
        start[28], start[29], start[30], start[31],
    ]));
    if blocks == 0 || block_bytes < DISC_HEADER_BYTES as u32 {
        return Err(DAMAGED);
    }
    let first = le64(&start[32..40]);
    // Block 0 ends where block 1 starts, or at the compressed size (one block).
    let end = if blocks > 1 {
        if file_len < GCZ_HEADER_BYTES + 16 {
            return Err(DiscError::Format("GCZ image is truncated"));
        }
        let mut next = [0u8; 8];
        reader.read_exact(&mut next)?; // offsets[1] follows offsets[0]: no seek
        le64(&next)
    } else {
        le64(&start[8..16])
    };
    let from = first & !GCZ_STORED_BIT;
    let stored = (end & !GCZ_STORED_BIT)
        .checked_sub(from)
        .filter(|len| *len >= 1)
        .ok_or(DAMAGED)?;
    if stored > MAX_GCZ_BLOCK_READ {
        return Err(DiscError::Format("GCZ block is too large"));
    }
    let at = (GCZ_HEADER_BYTES + 12 * blocks)
        .checked_add(from)
        .filter(|at| at.checked_add(stored).is_some_and(|e| e <= file_len))
        .ok_or(DiscError::Format("GCZ image is truncated"))?;
    reader.seek(SeekFrom::Start(at))?;
    let mut block = reader.by_ref().take(stored);
    let mut head = [0u8; DISC_HEADER_BYTES];
    let read = if first & GCZ_STORED_BIT != 0 {
        block.read_exact(&mut head)
    } else {
        ZlibDecoder::new(block).read_exact(&mut head)
    };
    read.map_err(|_| DiscError::Format("GCZ block does not decompress"))?;
    Ok(Some(head))
}
