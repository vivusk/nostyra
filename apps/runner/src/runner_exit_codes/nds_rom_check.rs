//! Nintendo DS dumps melonDS DS cannot play with its built-in system files, refused
//! before the load with a clear reason (exit 4): DSi-only titles (unit code 3; the
//! core boots unit-3 homebrew in DS mode and refuses only retail DSi titles, with a
//! log line) and encrypted dumps.
//!
//! The encrypted-dump rule mirrors melonds-ds `src/libretro/config/console.cpp`
//! L420–424 @f394adba exactly: a ROM is refused when it is not homebrew (melonDS
//! `NDSHeader::IsHomebrew`, `src/NDS_Header.h` L213–216 @16f127db: ARM9 ROM offset
//! < 0x4000 or game code `####`) and its secure area is not decrypted (the first
//! ARM9 word is `0xE7FFDEFF` and the word at +0x10 is not). The core has no upper
//! bound on the ARM9 offset, so neither has this check.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use system_catalog::NDS_DSI_ONLY;

/// Reason for an encrypted dump (the core needs native BIOS files for it).
pub(crate) const NDS_ENCRYPTED: &str =
    "This DS dump is encrypted; Nostyra plays decrypted dumps (No-Intro Decrypted)";

/// Header bytes the rule reads: game code 0x0C, unit code 0x12, ARM9 offset 0x20.
const HEADER_BYTES: usize = 0x24;
/// Bytes read at the ARM9 offset: the marker word and the word at +0x10.
const ARM9_HEAD_BYTES: usize = 0x14;
const DECRYPTED_MARKER: u32 = 0xE7FF_DEFF;
const HOMEBREW_OFFSET_LIMIT: u32 = 0x4000;
const UNIT_DSI_ONLY: u8 = 3;

fn word(bytes: &[u8], at: usize) -> u32 {
    let mut le = [0u8; 4];
    for (i, byte) in le.iter_mut().enumerate() {
        *byte = bytes.get(at + i).copied().unwrap_or(0);
    }
    u32::from_le_bytes(le)
}

/// ARM9 ROM offset, when `header` is not homebrew (only then the secure area counts).
pub(crate) fn retail_arm9_offset(header: &[u8]) -> Option<u32> {
    let offset = word(header, 0x20);
    let homebrew = offset < HOMEBREW_OFFSET_LIMIT || header.get(0x0C..0x10) == Some(b"####");
    (!homebrew).then_some(offset)
}

/// Why this dump cannot play, if it cannot. `arm9_head`: the bytes at the ARM9 ROM
/// offset (missing bytes count as 0, like the core's zero-padded ROM copy).
pub(crate) fn nds_dump_refusal(header: &[u8], arm9_head: &[u8]) -> Option<&'static str> {
    if header.get(0x12) == Some(&UNIT_DSI_ONLY) {
        return Some(NDS_DSI_ONLY);
    }
    retail_arm9_offset(header)?;
    let decrypted =
        word(arm9_head, 0) == DECRYPTED_MARKER && word(arm9_head, 0x10) != DECRYPTED_MARKER;
    (!decrypted).then_some(NDS_ENCRYPTED)
}

/// [`nds_dump_refusal`] on the file: the header, then (non-homebrew only) the bytes
/// at the ARM9 offset. Read errors refuse nothing (the core reports them).
pub(crate) fn check_nds_file(path: &Path) -> Option<&'static str> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut header = [0u8; HEADER_BYTES];
    file.read_exact(&mut header).ok()?;
    let mut arm9_head = Vec::with_capacity(ARM9_HEAD_BYTES);
    if let Some(offset) = retail_arm9_offset(&header) {
        file.seek(SeekFrom::Start(offset.into())).ok()?;
        file.take(ARM9_HEAD_BYTES as u64)
            .read_to_end(&mut arm9_head)
            .ok()?;
    }
    nds_dump_refusal(&header, &arm9_head)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Header with `code`, `unit` and ARM9 at `arm9`, plus the ARM9 head words.
    fn rom(code: &[u8; 4], unit: u8, arm9: u32, first: u32, plus_0x10: u32) -> Vec<u8> {
        let mut rom = vec![0u8; arm9 as usize + 0x200];
        rom[0x0C..0x10].copy_from_slice(code);
        rom[0x12] = unit;
        rom[0x20..0x24].copy_from_slice(&arm9.to_le_bytes());
        let at = arm9 as usize;
        rom[at..at + 4].copy_from_slice(&first.to_le_bytes());
        rom[at + 0x10..at + 0x14].copy_from_slice(&plus_0x10.to_le_bytes());
        rom
    }

    fn refusal(rom: &[u8]) -> Option<&'static str> {
        let arm9 = word(rom, 0x20) as usize;
        nds_dump_refusal(rom, rom.get(arm9..).unwrap_or_default())
    }

    #[test]
    fn homebrew_never_needs_the_marker() {
        assert_eq!(refusal(&rom(b"ZNSA", 0, 0x200, 0, 0)), None, "ARM9 @0x200");
        assert_eq!(
            refusal(&rom(b"ZNSA", 0, 0x3FFC, 0, 0)),
            None,
            "just under 0x4000"
        );
        assert_eq!(refusal(&rom(b"####", 0, 0x4000, 0, 0)), None, "code ####");
    }

    #[test]
    fn retail_layout_needs_a_decrypted_secure_area() {
        let decrypted = rom(b"ZNSB", 0, 0x4000, DECRYPTED_MARKER, 0x1234);
        assert_eq!(refusal(&decrypted), None, "test ROM B layout");
        let encrypted = rom(b"ZNSB", 0, 0x4000, 0x1234_5678, 0);
        assert_eq!(refusal(&encrypted), Some(NDS_ENCRYPTED));
        let twice = rom(b"ZNSB", 0, 0x4000, DECRYPTED_MARKER, DECRYPTED_MARKER);
        assert_eq!(refusal(&twice), Some(NDS_ENCRYPTED), "marker at +0x10 too");
    }

    #[test]
    fn no_upper_bound_on_the_arm9_offset() {
        let far = rom(b"ZNSB", 2, 0x8000, 0x1234_5678, 0);
        assert_eq!(refusal(&far), Some(NDS_ENCRYPTED));
        let far = rom(b"ZNSB", 2, 0x8000, DECRYPTED_MARKER, 0);
        assert_eq!(refusal(&far), None);
        let header = rom(b"ZNSB", 0, 0x4000, DECRYPTED_MARKER, 0);
        assert_eq!(
            nds_dump_refusal(&header, &[]),
            Some(NDS_ENCRYPTED),
            "ARM9 past the end reads as zeros"
        );
    }

    #[test]
    fn dsi_only_unit_code_is_refused_even_for_homebrew() {
        assert_eq!(refusal(&rom(b"ZNSA", 3, 0x200, 0, 0)), Some(NDS_DSI_ONLY));
        let retail = rom(b"ZNSB", 3, 0x4000, DECRYPTED_MARKER, 0);
        assert_eq!(refusal(&retail), Some(NDS_DSI_ONLY));
    }

    #[test]
    fn file_check_reads_only_what_the_rule_needs() {
        let temp = tempfile::tempdir().unwrap();
        let write = |name: &str, bytes: &[u8]| {
            let path = temp.path().join(name);
            std::fs::write(&path, bytes).unwrap();
            path
        };
        let ok = write("b.nds", &rom(b"ZNSB", 0, 0x4000, DECRYPTED_MARKER, 0));
        assert_eq!(check_nds_file(&ok), None);
        let enc = write("enc.nds", &rom(b"ZNSB", 0, 0x4000, 7, 0));
        assert_eq!(check_nds_file(&enc), Some(NDS_ENCRYPTED));
        let mut short = rom(b"ZNSB", 0, 0x4000, DECRYPTED_MARKER, 0);
        short.truncate(0x1000);
        let short = write("short.nds", &short);
        assert_eq!(check_nds_file(&short), Some(NDS_ENCRYPTED));
        assert_eq!(check_nds_file(&temp.path().join("missing.nds")), None);
    }
}
