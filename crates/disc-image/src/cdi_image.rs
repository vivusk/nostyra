//! DiscJuggler `.cdi` images (Dreamcast MIL-CD / self-boot dumps): the footer (last 8
//! bytes: version + header offset) points at a session/track table; track data is
//! stored back to back from byte 0 (pregap + track per entry). Layout as read by
//! Flycast's `cdipsr` (versions 2, 3 and 3.5); other versions are rejected.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::cdi_track_table::parse_table;
use crate::disc_sector_reader::FileTrackReader;
use crate::DiscError;

const V2: u32 = 0x8000_0004;
const V3: u32 = 0x8000_0005;
const V35: u32 = 0x8000_0006;
/// Real tables are a few KiB; refuse a header offset pointing further back.
const MAX_TABLE_BYTES: u64 = 1024 * 1024;
const BAD_CDI: &str = "not a valid DiscJuggler image";

/// One track of a DiscJuggler image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdiTrack {
    /// Session number (0-based).
    pub session: u16,
    /// 0 audio, 1 mode 1, 2 mode 2.
    pub mode: u32,
    /// Stored bytes per sector: 2048, 2336, 2352 or 2448.
    pub sector_size: u32,
    /// Disc LBA of the track's first sector (after its pregap).
    pub lba: u32,
    /// Sectors of track data (pregap excluded).
    pub length: u32,
    /// Byte offset of the track's first sector in the image.
    pub data_start: u64,
}

impl CdiTrack {
    pub fn is_data(&self) -> bool {
        self.mode != 0
    }

    /// Offset of the 2048-byte user data inside a stored sector.
    fn data_offset(&self) -> u64 {
        match (self.sector_size, self.mode) {
            (2048, _) => 0,
            (2336, _) => 8,
            (_, 1) => 16,
            _ => 24,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CdiImage {
    pub tracks: Vec<CdiTrack>,
}

impl CdiImage {
    /// Reads the track table of `path` (only the footer and the table are read).
    pub fn read(path: &Path) -> Result<Self, DiscError> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        if len < 8 {
            return Err(DiscError::Format(BAD_CDI));
        }
        file.seek(SeekFrom::Start(len - 8))?;
        let mut footer = [0u8; 8];
        file.read_exact(&mut footer)?;
        let version = u32::from_le_bytes([footer[0], footer[1], footer[2], footer[3]]);
        let offset = u64::from(u32::from_le_bytes([
            footer[4], footer[5], footer[6], footer[7],
        ]));
        let table_at = match version {
            V35 => len.checked_sub(offset),
            V2 | V3 => Some(offset),
            _ => return Err(DiscError::Format("unsupported DiscJuggler version")),
        }
        .filter(|at| offset != 0 && *at < len - 8 && len - 8 - at <= MAX_TABLE_BYTES)
        .ok_or(DiscError::Format(BAD_CDI))?;
        file.seek(SeekFrom::Start(table_at))?;
        let mut table = Vec::new();
        file.take(len - 8 - table_at).read_to_end(&mut table)?;
        parse_table(&table, version != V2).ok_or(DiscError::Format(BAD_CDI))
    }

    /// The track holding IP.BIN: the first data track of the last session that has a
    /// data track (MIL-CD: audio session first, data session last).
    pub fn boot_track(&self) -> Option<&CdiTrack> {
        let session = self
            .tracks
            .iter()
            .filter(|t| t.is_data())
            .map(|t| t.session)
            .max()?;
        self.tracks
            .iter()
            .find(|t| t.session == session && t.is_data())
    }

    /// A sector reader over one of this image's data tracks.
    pub(crate) fn open_track(path: &Path, track: &CdiTrack) -> Result<FileTrackReader, DiscError> {
        FileTrackReader::open_track(
            path,
            u64::from(track.sector_size),
            track.data_offset(),
            track.data_start,
            Some(u64::from(track.length)),
        )
    }
}

#[cfg(test)]
#[path = "cdi_image_tests.rs"]
mod tests;
