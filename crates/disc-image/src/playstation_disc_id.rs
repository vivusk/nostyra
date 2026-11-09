//! PlayStation disc identification from its first data track: the ISO9660 system id
//! `PLAYSTATION` plus Sony's licence text in sector 4 (PS2 CDs share the system id
//! but carry no licence text), the boot serial from `SYSTEM.CNF`, and the region the
//! licence text names. PlayStation 2: the same system id plus a `BOOT2` line (the
//! check rcheevos uses), its serial and the `VMODE` the BIOS picker needs; the
//! identity key (serial, else volume id + boot path) is built in `ps2_disc_probe`.

use system_catalog::Ps2VideoMode;

use crate::disc_sector_reader::DiscReader;
use crate::iso9660_root::{read_extent, PrimaryVolume};
use crate::DiscError;

/// Sector holding the licence text.
const LICENCE_SECTOR: u32 = 4;
const LICENCE_MARK: &[u8] = b"Sony Computer";
/// `SYSTEM.CNF` is a few lines; read at most one sector.
const MAX_SYSTEM_CNF: usize = 2048;
const SYSTEM_ID: &str = "PLAYSTATION";
/// Device prefix of a PS2 `BOOT2` path.
const PS2_BOOT_DEVICE: &str = "cdrom0:";

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
    Ok(pvd.system_id == SYSTEM_ID && licence_tail(reader)?.is_some())
}

/// Whether the image is a PlayStation 2 disc: `PLAYSTATION` volume whose root
/// `SYSTEM.CNF` has a `BOOT2` line. A PS1 disc passes [`is_playstation`] first in
/// the classifier, so the licence text is not checked here.
pub fn is_playstation2(reader: &mut dyn DiscReader) -> Result<bool, DiscError> {
    let pvd = PrimaryVolume::read(reader)?;
    if pvd.system_id != SYSTEM_ID {
        return Ok(false);
    }
    Ok(system_cnf(reader, &pvd)?.is_some_and(|cnf| boot2_exe(&cnf).is_some()))
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
    Ok(system_cnf(reader, &pvd)?.and_then(|cnf| boot_serial(&cnf)))
}

/// Text of the root `SYSTEM.CNF` (first 2 KiB), `None` when the volume has none.
pub fn system_cnf(
    reader: &mut dyn DiscReader,
    pvd: &PrimaryVolume,
) -> Result<Option<String>, DiscError> {
    let Some(extent) = pvd.find_root_file(reader, "SYSTEM.CNF")? else {
        return Ok(None);
    };
    let text = read_extent(reader, extent, MAX_SYSTEM_CNF)?;
    Ok(Some(String::from_utf8_lossy(&text).into_owned()))
}

/// The `BOOT` line's executable name, normalised. `BOOT2` (PS2) is not matched.
pub fn boot_serial(system_cnf: &str) -> Option<String> {
    let value = cnf_value(system_cnf, "BOOT")?;
    let path = strip_device(value, "cdrom:");
    let exe = path.rsplit(['\\', '/']).next()?;
    normalize_serial(exe.split(';').next()?)
}

/// The `BOOT2` path after `cdrom0:\`, upper-cased, `;1` kept
/// (`BOOT2 = cdrom0:\SLUS_203.12;1` → `SLUS_203.12;1`); `None` without a `BOOT2` line.
pub fn boot2_exe(system_cnf: &str) -> Option<String> {
    let value = cnf_value(system_cnf, "BOOT2")?;
    let path = strip_device(value, PS2_BOOT_DEVICE).trim_start_matches(['\\', '/']);
    (!path.is_empty()).then(|| path.to_ascii_uppercase())
}

/// The serial the `BOOT2` executable name carries (`SLUS_203.12;1` → `SLUS-20312`);
/// `None` for `BOOT.ELF`-style names.
pub fn boot2_serial(system_cnf: &str) -> Option<String> {
    let exe = boot2_exe(system_cnf)?;
    let name = exe.rsplit(['\\', '/']).next()?;
    normalize_serial(name.split(';').next()?)
}

/// The `VMODE` line (`NTSC` / `PAL`) of a PS2 `SYSTEM.CNF`.
pub fn disc_video_mode(system_cnf: &str) -> Option<Ps2VideoMode> {
    Ps2VideoMode::parse(cnf_value(system_cnf, "VMODE")?)
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

/// Trimmed value of the `KEY = value` line whose key is exactly `key`
/// (case-insensitive): `BOOT2 = …` is not a `BOOT` line.
fn cnf_value<'a>(system_cnf: &'a str, key: &str) -> Option<&'a str> {
    system_cnf.lines().find_map(|line| {
        let line = line.trim();
        let found = line
            .get(..key.len())
            .filter(|k| k.eq_ignore_ascii_case(key))?;
        line[found.len()..]
            .trim_start()
            .strip_prefix('=')
            .map(str::trim)
    })
}

/// `value` without a leading `device` prefix (case-insensitive).
fn strip_device<'a>(value: &'a str, device: &str) -> &'a str {
    value
        .get(..device.len())
        .filter(|p| p.eq_ignore_ascii_case(device))
        .map_or(value, |_| &value[device.len()..])
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
