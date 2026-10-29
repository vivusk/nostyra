//! Per-system file-format rules: how a file proves it belongs to a system
//! (`ContentProbe`) and which header bytes to skip before hashing (`HeaderRule`).

use crate::disc_probe::DiscSignature;
use crate::mega_drive_smd_layout::{SMD_BLOCK_BYTES, SMD_HEADER_BYTES};

/// Bytes read from the start of a file for content probes: enough for every probe in
/// the registry. The DS header CRC ends at 0x160 (Game Boy logo bytes: 0x108), so
/// 0x200 (512 bytes, one small read per probed file; extension-only systems read
/// nothing).
pub const PROBE_BYTES: usize = 0x200;

/// iNES header size and the optional trainer that follows it (flag 6 bit 2).
const INES_HEADER_BYTES: usize = 16;
const INES_TRAINER_BYTES: usize = 512;
const INES_TRAINER_FLAG: u8 = 0x04;

/// Content check for a file whose extension already matched: magic bytes in the first
/// [`PROBE_BYTES`], or a disc signature checked on the image by a
/// [`DiscProbe`](crate::DiscProbe). Shared extensions (`.bin`, `.iso`) are decided here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentProbe {
    /// No check: the extension alone decides.
    Any,
    /// `bytes` must appear at `offset` (`offset + bytes.len() <= PROBE_BYTES`).
    Magic {
        offset: usize,
        bytes: &'static [u8],
        /// Readable reason shown when the check fails ("missing iNES header").
        missing: &'static str,
    },
    /// Magic checked only for files with one of `exts` or `quiet_exts` (any of
    /// `offsets` may match); other extensions of the system pass (extension decides).
    /// A miss on a `quiet_exts` file means "not a ROM at all" (e.g. a Markdown
    /// `README.md`), see [`ContentProbe::is_quiet`].
    MagicForExtensions {
        exts: &'static [&'static str],
        quiet_exts: &'static [&'static str],
        offsets: &'static [usize],
        bytes: &'static [u8],
        missing: &'static str,
    },
    /// One of `patterns` must appear at `offset` (a format with several known
    /// layouts, e.g. the three N64 byte orders); a miss is a probe failure.
    MagicAny {
        offset: usize,
        patterns: &'static [&'static [u8]],
        missing: &'static str,
    },
    /// Disc image signature (sector reads, cue/m3u resolution) — see
    /// [`DiscProbe`](crate::DiscProbe).
    Disc(DiscSignature),
    /// Nintendo DS cartridge header: header CRC and code fields (`nds_header_probe`);
    /// a DSi-only game fails with its own reason.
    NdsHeader,
}

/// How to find the bytes DATs hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderRule {
    /// Hash the whole file.
    None,
    /// SNES copier header: `len % 1024 == 512` → skip the first 512 bytes
    /// (the same rule Snes9x applies when loading).
    SnesCopier512,
    /// NES iNES header: the whole file (header included) is the primary hash, as
    /// libretro-database lists headered `.nes` dumps; the bytes after the header
    /// (see [`ines_header_len`]) are a fallback hash for re-headered dumps.
    INes,
    /// Mega Drive: an SMD-interleaved file (512-byte copier header, 16 KiB blocks
    /// split into odd/even halves; see `mega_drive_smd_layout`) hashes as the raw
    /// image No-Intro lists, anything else as the whole file.
    MegaDriveSmd,
    /// Nintendo 64: `.v64` (16-bit swapped) and `.n64` (32-bit little-endian) dumps
    /// hash as the big-endian `.z64` image No-Intro lists (see `n64_byte_order`),
    /// so all three copies share one DAT entry and content id.
    N64ByteOrder,
}

/// Bytes skipped before hashing a file of `file_len` bytes, which is also what a DAT
/// `size` refers to. Never mixed with [`ines_header_len`] (NES fallback only).
///
/// `MegaDriveSmd` is decided by size only (`len % 16 KiB == 512`), while the hash
/// also checks the bytes (`SEGA` missing at 0x100): a raw dump of that size with
/// `SEGA` is hashed whole but rematched with size `len - 512`. Accepted: no real
/// cartridge has that size.
pub fn hash_skip_len(rule: HeaderRule, file_len: u64) -> usize {
    match rule {
        HeaderRule::SnesCopier512 if file_len % 1024 == 512 => 512,
        HeaderRule::MegaDriveSmd if file_len % SMD_BLOCK_BYTES == SMD_HEADER_BYTES => 512,
        HeaderRule::None
        | HeaderRule::SnesCopier512
        | HeaderRule::INes
        | HeaderRule::MegaDriveSmd
        | HeaderRule::N64ByteOrder => 0,
    }
}

/// Length of the iNES header plus trainer at the start of `bytes` (0 when shorter
/// than a header). Used only for the NES headerless fallback hash.
pub fn ines_header_len(bytes: &[u8]) -> usize {
    if bytes.len() < INES_HEADER_BYTES {
        return 0;
    }
    if bytes[6] & INES_TRAINER_FLAG != 0 {
        INES_HEADER_BYTES + INES_TRAINER_BYTES
    } else {
        INES_HEADER_BYTES
    }
}

#[cfg(test)]
#[path = "rom_header_rules_tests.rs"]
mod tests;
