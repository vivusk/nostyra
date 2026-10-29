//! GameCube / Wii executables (`.dol`, `.elf`): structure checks and the GC/Wii
//! split Dolphin itself uses (`DolReader` / `ElfReader::IsWii`, libretro/dolphin @
//! 3aac3f05): an `mtspr HID4, rS` word (`0x7C13FBA6` under mask `0xFC1FFFFF`) in
//! code means Wii (HID4 only exists on the Wii's Broadway CPU); a DOL section loaded
//! into Wii MEM2 (0x90000000+) also means Wii. Executables carry no game ID (their
//! identity is the file hash). Code is scanned streaming, at most
//! [`MAX_SCAN_BYTES`] (real executables are a few MB).
//!
//! - DOL: 0x100-byte BE header — file offsets of 7 text + 11 data sections @0x00,
//!   load addresses @0x48, sizes @0x90, entry point @0xE0. Valid = ≥ 1 text section,
//!   every non-empty section inside the file and inside MEM1 (0x80000000–0x81800000)
//!   or MEM2 (0x90000000–0x94000000), entry point inside a text section.
//! - ELF: `7F 'ELF'`, 32-bit, big-endian, machine 20 (PowerPC), program header
//!   table and every segment inside the file, ≥ 1 executable `PT_LOAD` segment.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::ops::RangeInclusive;
use std::path::Path;

use crate::nintendo_disc_header::NintendoDiscKind;
use crate::{DiscError, DOL_EXT, ELF_EXT};

/// Most code bytes searched for the HID4 word.
pub const MAX_SCAN_BYTES: u64 = 32 * 1024 * 1024;

const HID4_WORD: u32 = 0x7C13_FBA6;
const HID4_MASK: u32 = 0xFC1F_FFFF;
const MEM1: RangeInclusive<u64> = 0x8000_0000..=0x8180_0000;
const MEM2: RangeInclusive<u64> = 0x9000_0000..=0x9400_0000;
const DOL_HEADER_BYTES: usize = 0x100;
const DOL_TEXT_SECTIONS: usize = 7;
const DOL_SECTIONS: usize = DOL_TEXT_SECTIONS + 11;
const ELF_HEADER_BYTES: usize = 52;
const EM_PPC: u16 = 20;
const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const MAX_PROGRAM_HEADERS: usize = 64;
/// Standard ELF32 program header size; larger entries (≤ 64 bytes) are accepted.
const PHDR_BYTES: RangeInclusive<usize> = 32..=64;
const SCAN_CHUNK: usize = 64 * 1024;

/// Code bytes of an executable: (file offset, size).
type CodeRanges = Vec<(u64, u64)>;

/// Whether `ext` is an executable extension (`dol` / `elf`, any case).
pub fn is_executable_ext(ext: &str) -> bool {
    ext.eq_ignore_ascii_case(DOL_EXT) || ext.eq_ignore_ascii_case(ELF_EXT)
}

/// GC or Wii for the `.dol` / `.elf` at `path`; `None` when it is not a valid
/// PowerPC executable of that kind (or not an executable extension).
pub fn read_executable_kind(path: &Path) -> Result<Option<NintendoDiscKind>, DiscError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if !is_executable_ext(ext) {
        return Ok(None);
    }
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    executable_kind_from(&mut file, ext, file_len)
}

/// [`read_executable_kind`] over any reader (`file_len` = its total length).
pub fn executable_kind_from<R: Read + Seek>(
    reader: &mut R,
    ext: &str,
    file_len: u64,
) -> Result<Option<NintendoDiscKind>, DiscError> {
    let parsed = if ext.eq_ignore_ascii_case(DOL_EXT) {
        dol_code(reader, file_len)?
    } else {
        elf_code(reader, file_len)?
    };
    let Some((code, in_mem2)) = parsed else {
        return Ok(None);
    };
    let wii = in_mem2 || has_hid4_word(reader, &code)?;
    Ok(Some(match wii {
        true => NintendoDiscKind::Wii,
        false => NintendoDiscKind::GameCube,
    }))
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn read_head<R: Read + Seek>(
    reader: &mut R,
    buf: &mut [u8],
    file_len: u64,
) -> Result<bool, DiscError> {
    if file_len < buf.len() as u64 {
        return Ok(false);
    }
    reader.seek(SeekFrom::Start(0))?;
    reader.read_exact(buf)?;
    Ok(true)
}

/// Text sections + "a section lives in MEM2", or `None` for a broken table.
fn dol_code<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
) -> Result<Option<(CodeRanges, bool)>, DiscError> {
    let mut h = [0u8; DOL_HEADER_BYTES];
    if !read_head(reader, &mut h, file_len)? {
        return Ok(None);
    }
    let word = |index: usize| u64::from(be32(&h, index * 4));
    let entry = u64::from(be32(&h, 0xE0));
    let (mut text, mut in_mem2, mut entry_in_text) = (Vec::new(), false, false);
    for i in 0..DOL_SECTIONS {
        let (offset, addr, size) = (word(i), word(DOL_SECTIONS + i), word(2 * DOL_SECTIONS + i));
        if size == 0 {
            continue;
        }
        let (in_file, end) = (offset + size <= file_len, addr + size);
        let in_ram = |ram: &RangeInclusive<u64>| ram.contains(&addr) && ram.contains(&end);
        if !in_file || !(in_ram(&MEM1) || in_ram(&MEM2)) {
            return Ok(None);
        }
        in_mem2 |= in_ram(&MEM2);
        if i < DOL_TEXT_SECTIONS {
            entry_in_text |= (addr..end).contains(&entry);
            text.push((offset, size));
        }
    }
    Ok((!text.is_empty() && entry_in_text).then_some((text, in_mem2)))
}

/// Executable `PT_LOAD` segments of a big-endian PowerPC ELF32, or `None`.
fn elf_code<R: Read + Seek>(
    reader: &mut R,
    file_len: u64,
) -> Result<Option<(CodeRanges, bool)>, DiscError> {
    let mut h = [0u8; ELF_HEADER_BYTES];
    if !read_head(reader, &mut h, file_len)? {
        return Ok(None);
    }
    let be16 = |at: usize| u16::from_be_bytes([h[at], h[at + 1]]);
    let is_ppc32_be = &h[..4] == b"\x7FELF" && h[4] == 1 && h[5] == 2 && be16(18) == EM_PPC;
    let (table_at, entry_bytes, count) = (
        u64::from(be32(&h, 28)),
        usize::from(be16(42)),
        usize::from(be16(44)),
    );
    let table_bytes = (entry_bytes * count) as u64;
    if !is_ppc32_be
        || !PHDR_BYTES.contains(&entry_bytes)
        || !(1..=MAX_PROGRAM_HEADERS).contains(&count)
        || table_at + table_bytes > file_len
    {
        return Ok(None);
    }
    let mut table = vec![0u8; entry_bytes * count];
    reader.seek(SeekFrom::Start(table_at))?;
    reader.read_exact(&mut table)?;
    let mut code = Vec::new();
    for phdr in table.chunks_exact(entry_bytes) {
        let (offset, size) = (u64::from(be32(phdr, 4)), u64::from(be32(phdr, 16)));
        if offset + size > file_len {
            return Ok(None);
        }
        if be32(phdr, 0) == PT_LOAD && be32(phdr, 24) & PF_X != 0 && size > 0 {
            code.push((offset, size));
        }
    }
    Ok((!code.is_empty()).then_some((code, false)))
}

/// Whether a 4-byte-aligned word of any code range is `mtspr HID4, rS`.
fn has_hid4_word<R: Read + Seek>(reader: &mut R, code: &[(u64, u64)]) -> Result<bool, DiscError> {
    let mut budget = MAX_SCAN_BYTES;
    let mut chunk = vec![0u8; SCAN_CHUNK];
    for &(offset, size) in code {
        reader.seek(SeekFrom::Start(offset))?;
        let mut left = size.min(budget) & !3;
        budget -= left;
        while left > 0 {
            let len = left.min(SCAN_CHUNK as u64) as usize;
            reader.read_exact(&mut chunk[..len])?;
            if chunk[..len]
                .as_chunks::<4>()
                .0
                .iter()
                .any(|w| u32::from_be_bytes(*w) & HID4_MASK == HID4_WORD)
            {
                return Ok(true);
            }
            left -= len as u64;
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "dol_elf_executable_tests.rs"]
mod tests;
