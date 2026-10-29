//! Minimal ISO9660: the primary volume descriptor (sector 16) and lookup of a file by
//! walking directory records (root `SYSTEM.CNF` on PS1, `PSP_GAME/PARAM.SFO` on PSP)
//! without the path table.

use crate::disc_sector_reader::{DiscReader, SECTOR_DATA_BYTES};
use crate::DiscError;

/// Sector of the primary volume descriptor.
pub const PVD_SECTOR: u32 = 16;
/// Directories of real discs span a few sectors; cap the walk.
const MAX_DIR_SECTORS: u32 = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryVolume {
    /// Space-trimmed system identifier (`PLAYSTATION` on PS1/PS2 discs).
    pub system_id: String,
    pub volume_id: String,
    root: Extent,
}

/// A file's location: first sector and byte size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub lba: u32,
    pub size: u32,
}

impl PrimaryVolume {
    /// Parses sector 16; fails when it is not an ISO9660 primary volume descriptor.
    pub fn read(reader: &mut dyn DiscReader) -> Result<Self, DiscError> {
        let pvd = reader.read_user_data(PVD_SECTOR)?;
        if pvd[0] != 1 || &pvd[1..6] != b"CD001" {
            return Err(DiscError::Format("no ISO9660 volume (not a data disc)"));
        }
        Ok(Self {
            system_id: ascii_field(&pvd[8..40]),
            volume_id: ascii_field(&pvd[40..72]),
            root: record_extent(&pvd[156..190]),
        })
    }

    /// Finds `name` (case-insensitive, `;1` version suffix ignored) in the root dir.
    pub fn find_root_file(
        &self,
        reader: &mut dyn DiscReader,
        name: &str,
    ) -> Result<Option<Extent>, DiscError> {
        find_in_dir(reader, self.root, name)
    }

    /// Finds a file or directory by path components from the root
    /// (`["PSP_GAME", "PARAM.SFO"]`); `None` when any component is missing.
    pub fn find_path(
        &self,
        reader: &mut dyn DiscReader,
        path: &[&str],
    ) -> Result<Option<Extent>, DiscError> {
        let mut extent = self.root;
        for name in path {
            match find_in_dir(reader, extent, name)? {
                Some(found) => extent = found,
                None => return Ok(None),
            }
        }
        Ok(Some(extent))
    }
}

/// Finds `name` among the records of the directory at `dir`.
pub fn find_in_dir(
    reader: &mut dyn DiscReader,
    dir: Extent,
    name: &str,
) -> Result<Option<Extent>, DiscError> {
    let sectors = dir.size.div_ceil(SECTOR_DATA_BYTES as u32);
    for i in 0..sectors.min(MAX_DIR_SECTORS) {
        let sector = reader.read_user_data(dir.lba.saturating_add(i))?;
        let mut pos = 0;
        // Records never cross a sector; a 0 length byte pads to the next one.
        while pos + 33 <= sector.len() && sector[pos] != 0 {
            let record = &sector[pos..(pos + usize::from(sector[pos])).min(sector.len())];
            if record.len() >= 33 {
                let name_len = usize::from(record[32]);
                let id = record.get(33..33 + name_len).unwrap_or(&[]);
                if same_name(id, name) {
                    return Ok(Some(record_extent(record)));
                }
            }
            pos += usize::from(sector[pos]);
        }
    }
    Ok(None)
}

/// Reads up to `max_bytes` of a file's content.
pub fn read_extent(
    reader: &mut dyn DiscReader,
    extent: Extent,
    max_bytes: usize,
) -> Result<Vec<u8>, DiscError> {
    let len = (extent.size as usize).min(max_bytes);
    let mut out = Vec::with_capacity(len);
    let mut lba = extent.lba;
    while out.len() < len {
        let sector = reader.read_user_data(lba)?;
        let take = (len - out.len()).min(SECTOR_DATA_BYTES);
        out.extend_from_slice(&sector[..take]);
        lba = lba
            .checked_add(1)
            .ok_or(DiscError::Format("file extent past the end of the disc"))?;
    }
    Ok(out)
}

/// Extent of a directory record (both-endian fields: little-endian half used).
fn record_extent(record: &[u8]) -> Extent {
    let le = |at: usize| {
        u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
    };
    Extent {
        lba: le(2),
        size: le(10),
    }
}

/// `SYSTEM.CNF;1` matches `system.cnf`.
fn same_name(id: &[u8], wanted: &str) -> bool {
    let id = id.split(|&b| b == b';').next().unwrap_or(id);
    let id = id.strip_suffix(b".").unwrap_or(id);
    id.eq_ignore_ascii_case(wanted.as_bytes())
}

fn ascii_field(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .trim_end_matches([' ', '\0'])
        .to_owned()
}

#[cfg(test)]
#[path = "iso9660_root_tests.rs"]
mod tests;
