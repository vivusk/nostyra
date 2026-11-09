//! PlayStation 2 BIOS recognition the way PCSX2 does it (`IsBIOS`): the image holds
//! a ROMDIR table — 16-byte records `{name[10], ext_info_size u16, file_size u32}`
//! starting with `RESET` — and one record is `ROMVER`, 14 ASCII bytes such as
//! `0160AC20020207` (v1.60, region A = USA, C = console, 2002-02-07). No md5 table:
//! any 4–8 MiB dump with that entry boots on the core, so the host accepts the same
//! set (M18 research §4.1; spike: the user's `.ROM1` has `RESET` but no `ROMVER`,
//! `.DIFF` has no `RESET` — both invalid, as on the core).

use crate::firmware_status::ANY_REGION;

/// Smallest PS2 BIOS image (every retail dump is 4 MiB).
pub const PS2_BIOS_MIN_BYTES: u64 = 4 * 1024 * 1024;
/// Largest PS2 BIOS image the core (and the import read cap) accepts.
pub const PS2_BIOS_MAX_BYTES: u64 = 8 * 1024 * 1024;

const RECORD_BYTES: usize = 16;
const ROMVER_BYTES: usize = 14;

/// Decoded `ROMVER` entry. Ordered by version then date (field order), so the
/// newest dump compares greatest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ps2BiosVersion {
    pub major: u8,
    pub minor: u8,
    /// `YYYY-MM-DD`.
    pub date: String,
    /// Region letter (`J` / `A` / `E` / …).
    pub region: char,
    /// `C` = retail console; `D` = development / arcade unit.
    pub console: bool,
}

impl Ps2BiosVersion {
    /// "v1.60 · USA · 2002-02-07" (" · devel" appended for non-console units).
    pub fn label(&self) -> String {
        let devel = if self.console { "" } else { " · devel" };
        format!(
            "v{}.{:02} · {} · {}{devel}",
            self.major,
            self.minor,
            region_name(self.region),
            self.date
        )
    }

    /// Settings / picker region: `JP` / `US` / `EU` for J / A / E, else any-region.
    pub fn region_code(&self) -> &'static str {
        match self.region {
            'J' => "JP",
            'A' => "US",
            'E' => "EU",
            _ => ANY_REGION,
        }
    }

    /// v1.00 (SCPH-10000) has memory-card bugs the core cannot hide: shown as a
    /// warning in Settings, never refused.
    pub fn is_discouraged(&self) -> bool {
        (self.major, self.minor) == (1, 0)
    }
}

/// Region names PCSX2 prints for the `ROMVER` letter.
fn region_name(letter: char) -> &'static str {
    match letter {
        'J' => "Japan",
        'A' => "USA",
        'E' => "Europe",
        'H' => "Hong Kong",
        'C' => "China",
        'T' => "T10K",
        'X' => "Test",
        'P' => "Free",
        _ => "region ?",
    }
}

/// The dump's `ROMVER`, or `None` when `bytes` is not a PS2 BIOS (size outside
/// 4–8 MiB, no `RESET` record, no `ROMVER` record, or a malformed version string).
pub fn parse_ps2_bios(bytes: &[u8]) -> Option<Ps2BiosVersion> {
    let len = bytes.len() as u64;
    if !(PS2_BIOS_MIN_BYTES..=PS2_BIOS_MAX_BYTES).contains(&len) {
        return None;
    }
    let table = find_reset_record(bytes)?;
    let raw = romver_bytes(bytes, table)?;
    decode_romver(raw)
}

/// Offset of the first record-aligned `RESET` record (the ROMDIR table start).
fn find_reset_record(bytes: &[u8]) -> Option<usize> {
    (0..bytes.len())
        .step_by(RECORD_BYTES)
        .find(|&at| bytes.get(at..at + 10) == Some(b"RESET\0\0\0\0\0"))
}

/// Walks the record table at `table`: file offsets accumulate (each size rounded up
/// to 16) from the image start; the `ROMVER` record's bytes are returned.
fn romver_bytes(bytes: &[u8], table: usize) -> Option<&[u8]> {
    let mut file_offset = 0usize;
    for at in (table..bytes.len()).step_by(RECORD_BYTES) {
        let record = bytes.get(at..at + RECORD_BYTES)?;
        let name_end = record[..10].iter().position(|&b| b == 0).unwrap_or(10);
        if name_end == 0 {
            return None; // end of table
        }
        let size = u32::from_le_bytes([record[12], record[13], record[14], record[15]]) as usize;
        if &record[..name_end] == b"ROMVER" {
            return bytes.get(file_offset..file_offset + ROMVER_BYTES);
        }
        file_offset = file_offset.checked_add(size.checked_add(0xF)? & !0xF)?;
    }
    None
}

/// `0160AC20020207` → v1.60, A, console, 2002-02-07.
fn decode_romver(raw: &[u8]) -> Option<Ps2BiosVersion> {
    let raw = raw.get(..ROMVER_BYTES)?;
    if !raw.is_ascii() {
        return None; // byte-index splits below must land on char boundaries
    }
    let text = std::str::from_utf8(raw).ok()?;
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let (version, rest) = text.split_at(4);
    let (flags, date) = rest.split_at(2);
    if !digits(version) || !digits(date) {
        return None;
    }
    let mut flags = flags.chars();
    let region = flags.next()?;
    let unit = flags.next()?;
    if !region.is_ascii_alphanumeric() || !unit.is_ascii_alphanumeric() {
        return None;
    }
    Some(Ps2BiosVersion {
        major: version[..2].parse().ok()?,
        minor: version[2..].parse().ok()?,
        date: format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]),
        region,
        console: unit == 'C',
    })
}

#[cfg(test)]
#[path = "ps2_bios_romdir_tests.rs"]
mod tests;
