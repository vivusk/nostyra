//! PlayStation disc identification from its first data track: the ISO9660 system id
//! `PLAYSTATION` plus Sony's licence text in sector 4 (PS2 CDs share the system id
//! but carry no licence text), the boot serial from `SYSTEM.CNF`, and the region the
//! licence text names.

use crate::disc_sector_reader::DiscReader;
use crate::iso9660_root::{read_extent, PrimaryVolume};
use crate::DiscError;

/// Sector holding the licence text.
const LICENCE_SECTOR: u32 = 4;
const LICENCE_MARK: &[u8] = b"Sony Computer";
/// `SYSTEM.CNF` is a few lines; read at most one sector.
const MAX_SYSTEM_CNF: usize = 2048;

/// Disc region, from the licence text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscRegion {
    Japan,
    America,
    Europe,
}

/// Whether the image is a PlayStation (PS1) disc.
pub fn is_playstation(reader: &mut dyn DiscReader) -> Result<bool, DiscError> {
    let pvd = PrimaryVolume::read(reader)?;
    Ok(pvd.system_id == "PLAYSTATION" && licence_tail(reader)?.is_some())
}

/// Region named by the licence text (`…Entertainment Inc.` = Japan, `Amer ica` /
/// `of America` = America, `Euro pe` = Europe); `None` without licence text.
pub fn region(reader: &mut dyn DiscReader) -> Result<Option<DiscRegion>, DiscError> {
    let Some(tail) = licence_tail(reader)? else {
        return Ok(None);
    };
    let tail: Vec<u8> = tail.into_iter().filter(|b| *b != b' ').collect();
    let tail = tail.strip_prefix(b"Entertainment").unwrap_or(&tail);
    Ok(if tail.starts_with(b"Inc") {
        Some(DiscRegion::Japan)
    } else if tail.starts_with(b"Amer") || tail.starts_with(b"ofAmer") {
        Some(DiscRegion::America)
    } else if tail.starts_with(b"Euro") {
        Some(DiscRegion::Europe)
    } else {
        None
    })
}

/// Boot serial from `SYSTEM.CNF` (`BOOT = cdrom:\SLUS_012.34;1` → `SLUS-01234`);
/// `None` when the disc boots `PSX.EXE` (no `SYSTEM.CNF`) or the line is unreadable.
pub fn serial(reader: &mut dyn DiscReader) -> Result<Option<String>, DiscError> {
    let pvd = PrimaryVolume::read(reader)?;
    let Some(extent) = pvd.find_root_file(reader, "SYSTEM.CNF")? else {
        return Ok(None);
    };
    let text = read_extent(reader, extent, MAX_SYSTEM_CNF)?;
    Ok(boot_serial(&String::from_utf8_lossy(&text)))
}

/// The `BOOT` line's executable name, normalised. `BOOT2` (PS2) is not matched.
pub fn boot_serial(system_cnf: &str) -> Option<String> {
    let value = system_cnf.lines().find_map(|line| {
        let line = line.trim();
        let keyword = line.get(..4).filter(|k| k.eq_ignore_ascii_case("BOOT"))?;
        line[keyword.len()..].trim_start().strip_prefix('=')
    })?;
    let value = value.trim();
    let path = value
        .get(..6)
        .filter(|p| p.eq_ignore_ascii_case("cdrom:"))
        .map_or(value, |_| &value[6..]);
    let exe = path.rsplit(['\\', '/']).next()?;
    normalize_serial(exe.split(';').next()?)
}

/// `SLUS_012.34` / `SLUS01234` / `slus-01234` → `SLUS-01234`: alphanumerics only,
/// upper-cased, a dash before the first digit (PCSX ReARMed and Redump form).
pub fn normalize_serial(raw: &str) -> Option<String> {
    let clean: String = raw
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect();
    let split = clean.find(|c: char| c.is_ascii_digit())?;
    let (prefix, digits) = clean.split_at(split);
    let valid = !prefix.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
    valid.then(|| format!("{prefix}-{digits}"))
}

/// Bytes after "Sony Computer" in sector 4, if present.
fn licence_tail(reader: &mut dyn DiscReader) -> Result<Option<Vec<u8>>, DiscError> {
    let sector = reader.read_user_data(LICENCE_SECTOR)?;
    Ok(sector
        .windows(LICENCE_MARK.len())
        .position(|w| w == LICENCE_MARK)
        .map(|at| {
            let start = at + LICENCE_MARK.len();
            sector[start..(start + 32).min(sector.len())].to_vec()
        }))
}

#[cfg(test)]
#[path = "playstation_disc_id_tests.rs"]
mod tests;
