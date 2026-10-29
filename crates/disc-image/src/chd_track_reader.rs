//! CD-ROM CHD (MAME format) via the `chd` crate. Frames are 2448 bytes (2352 sector +
//! 96 subcode) packed into hunks; track 1's `CHT2`/`CHTR` metadata gives its type,
//! which fixes where the user data sits inside a frame. Only track 1 is read (the
//! PlayStation data track).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use chd::metadata::Metadata;
use chd::Chd;

use crate::chd_gd_tracks::{track_field, track_table, ChdTrackEntry};
use crate::disc_sector_reader::{DiscReader, SectorData, SECTOR_DATA_BYTES};
use crate::DiscError;

/// Bytes per CD frame in a CHD (sector + subcode).
pub const CHD_FRAME_BYTES: u64 = 2448;
/// chdman CD hunks are 8 frames (19 584 B); refuse absurd header values before
/// allocating a hunk buffer.
const MAX_HUNK_BYTES: u64 = 1024 * 1024;

pub struct ChdTrackReader {
    chd: Chd<BufReader<File>>,
    hunk_bytes: u64,
    data_offset: u64,
    /// Image frame of the track's sector 0 (after a stored pregap).
    first_frame: u64,
    frames: u64,
    hunk_buf: Vec<u8>,
    cmp_buf: Vec<u8>,
    cached_hunk: Option<u32>,
}

impl ChdTrackReader {
    /// Track 1 (the PlayStation data track).
    pub fn open(path: &Path) -> Result<Self, DiscError> {
        Self::open_track(path, |tracks| tracks.iter().find(|t| t.number == 1))
    }

    /// The track `pick` chooses from the image's track table (CD or GD-ROM metadata).
    pub(crate) fn open_track(
        path: &Path,
        pick: impl FnOnce(&[ChdTrackEntry]) -> Option<&ChdTrackEntry>,
    ) -> Result<Self, DiscError> {
        let mut chd = Chd::open(BufReader::new(File::open(path)?), None)?;
        let hunk_bytes = u64::from(chd.header().hunk_size());
        if hunk_bytes == 0 || hunk_bytes > MAX_HUNK_BYTES || hunk_bytes % CHD_FRAME_BYTES != 0 {
            return Err(DiscError::Format("CHD is not a CD image (hunk size)"));
        }
        let metas: Vec<Metadata> = chd.metadata_refs().try_into()?;
        let tracks = track_table(&metas);
        if tracks.is_empty() {
            return Err(DiscError::Format("CHD has no CD track metadata"));
        }
        let track = pick(&tracks).ok_or(DiscError::NoDataTrack)?;
        let layout = TrackLayout::parse(&track.text)?;
        let hunk_buf = chd.get_hunksized_buffer();
        Ok(Self {
            hunk_bytes,
            data_offset: layout.data_offset,
            first_frame: track.first_frame + layout.first_frame,
            frames: layout.frames,
            hunk_buf,
            cmp_buf: Vec::new(),
            cached_hunk: None,
            chd,
        })
    }
}

impl DiscReader for ChdTrackReader {
    fn read_user_data(&mut self, lba: u32) -> Result<SectorData, DiscError> {
        if u64::from(lba) >= self.frames {
            return Err(DiscError::SectorOutOfRange(lba));
        }
        let (hunk, offset) = frame_location(self.first_frame + u64::from(lba), self.hunk_bytes);
        if self.cached_hunk != Some(hunk) {
            self.cached_hunk = None;
            self.chd
                .hunk(hunk)?
                .read_hunk_in(&mut self.cmp_buf, &mut self.hunk_buf)?;
            self.cached_hunk = Some(hunk);
        }
        let start = usize::try_from(offset + self.data_offset)
            .map_err(|_| DiscError::Format("CHD frame offset overflow"))?;
        let mut data = [0u8; SECTOR_DATA_BYTES];
        let src = self
            .hunk_buf
            .get(start..start + SECTOR_DATA_BYTES)
            .ok_or(DiscError::SectorOutOfRange(lba))?;
        data.copy_from_slice(src);
        Ok(data)
    }

    fn sector_count(&self) -> u64 {
        self.frames
    }
}

/// Hunk number and byte offset of frame `frame`.
pub(crate) fn frame_location(frame: u64, hunk_bytes: u64) -> (u32, u64) {
    let byte = frame * CHD_FRAME_BYTES;
    // Hunk counts of a CD image (< 1 GB / 19584 B) always fit in u32.
    ((byte / hunk_bytes) as u32, byte % hunk_bytes)
}

/// Where a data track's user data lives, from its metadata text
/// (`TRACK:1 TYPE:MODE2_RAW SUBTYPE:NONE FRAMES:1234 PREGAP:150 PGTYPE:VMODE2_RAW …`).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TrackLayout {
    pub data_offset: u64,
    pub first_frame: u64,
    pub frames: u64,
}

impl TrackLayout {
    pub(crate) fn parse(text: &str) -> Result<Self, DiscError> {
        let data_offset = match track_field(text, "TYPE") {
            Some("MODE1" | "MODE2_FORM1") => 0,
            Some("MODE2" | "MODE2_FORM_MIX") => 8,
            Some("MODE1_RAW") => 16,
            Some("MODE2_RAW") => 24,
            _ => return Err(DiscError::Format("CHD track is not a data track")),
        };
        let number = |key| track_field(text, key).and_then(|v| v.parse::<u64>().ok());
        let frames = number("FRAMES").ok_or(DiscError::Format("CHD track has no FRAMES"))?;
        // A pregap is stored in the image only when its type starts with 'V'.
        let stored_pregap = track_field(text, "PGTYPE").is_some_and(|t| t.starts_with('V'));
        let first_frame = if stored_pregap {
            number("PREGAP").unwrap_or(0)
        } else {
            0
        };
        Ok(Self {
            data_offset,
            first_frame,
            frames: frames.saturating_sub(first_frame),
        })
    }
}

#[cfg(test)]
#[path = "chd_track_reader_tests.rs"]
mod tests;
