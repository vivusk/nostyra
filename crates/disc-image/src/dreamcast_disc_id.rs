//! Dreamcast disc identification from IP.BIN, the first sector of the boot data track
//! (GD-ROM: LBA 45000, track 3; MIL-CD: the last session's first data track):
//! hardware id `SEGA SEGAKATANA`, product number (the serial Redump lists), version,
//! area symbols (`J`/`U`/`E` = Japan / USA / Europe) and title.

use std::path::Path;

use crate::disc_sector_reader::{DiscReader, SectorData};
use crate::dreamcast_ip_track::open_ip_bin_track;
use crate::DiscError;

const HARDWARE_ID: &[u8] = b"SEGA SEGAKATANA";

/// The identifying fields of a Dreamcast IP.BIN (text fields trimmed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpBin {
    /// Product number, e.g. `MK-51000`, `HDR-0001`, `T-8101N`.
    pub product: String,
    /// Product version, e.g. `V1.005`.
    pub version: String,
    /// Disc number field, e.g. `1/2`.
    pub disc: String,
    /// Area symbols, e.g. `JUE`, `U`.
    pub area: String,
    /// Software title (ASCII; other bytes replaced).
    pub title: String,
}

impl IpBin {
    /// Parses sector 0 of the boot track; `None` without the Dreamcast hardware id.
    pub fn parse(sector: &SectorData) -> Option<Self> {
        if !sector.starts_with(HARDWARE_ID) {
            return None;
        }
        let field = |at: usize, len: usize| {
            String::from_utf8_lossy(&sector[at..at + len])
                .trim_matches(|c: char| c.is_whitespace() || c == '\0')
                .to_owned()
        };
        Some(Self {
            product: field(0x40, 10),
            version: field(0x4A, 6),
            disc: field(0x2B, 5),
            area: field(0x30, 8),
            title: field(0x80, 128),
        })
    }

    /// Whether the area symbols name `region` (`J`, `U` or `E`).
    pub fn has_area(&self, region: char) -> bool {
        self.area.contains(region)
    }
}

/// IP.BIN read through an open boot-track reader; `None` when sector 0 is not a
/// Dreamcast IP.BIN.
pub fn ip_bin_from(reader: &mut dyn DiscReader) -> Result<Option<IpBin>, DiscError> {
    Ok(IpBin::parse(&reader.read_user_data(0)?))
}

/// IP.BIN of the disc image at `path` (`.gdi`, `.cue`, `.chd`, `.cdi`, or a bare
/// track file); `None` when the boot track is not a Dreamcast IP.BIN.
pub fn read_ip_bin(path: &Path) -> Result<Option<IpBin>, DiscError> {
    let mut reader = open_ip_bin_track(path)?;
    ip_bin_from(reader.as_mut())
}

#[cfg(test)]
#[path = "dreamcast_disc_id_tests.rs"]
mod tests;
