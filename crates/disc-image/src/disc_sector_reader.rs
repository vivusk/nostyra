//! Reads the 2048-byte user data of one sector at a time, whatever the image layout:
//! raw 2352-byte sectors (MODE1 data at byte 16, MODE2 at 24), plain 2048-byte ISO
//! sectors, CSO, or CHD (CD/GD tracks or DVD). Only the sectors asked for are read (no whole-image loads).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::chd_dvd_reader::open_chd;
use crate::cso_image::CsoReader;
use crate::cue_sheet::{CueSheet, CueTrack, TrackMode};
use crate::{is_cue_ext, is_m3u_ext, DiscError, CHD_EXT, CSO_EXT, PBP_EXT};

/// User data bytes per sector.
pub const SECTOR_DATA_BYTES: usize = 2048;
/// Raw CD sector size.
pub const RAW_SECTOR_BYTES: u64 = 2352;
/// Sync pattern opening every raw data sector.
const SYNC: [u8; 12] = [
    0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0,
];

/// One sector's user data.
pub type SectorData = [u8; SECTOR_DATA_BYTES];

/// Random access to the user data of a data track, by logical sector (0 = first
/// sector of the track).
pub trait DiscReader {
    fn read_user_data(&mut self, lba: u32) -> Result<SectorData, DiscError>;
    /// Sectors readable from sector 0 on.
    fn sector_count(&self) -> u64;
}

/// A file of fixed-size sectors with user data at a fixed offset inside each.
pub struct FileTrackReader {
    file: File,
    sector_bytes: u64,
    data_offset: u64,
    /// Byte offset of track sector 0 (a cue `INDEX 01` past a stored pregap, a later
    /// track inside a DiscJuggler image).
    base: u64,
    /// Sectors from `base` to the end of the file (or of the track).
    sectors: u64,
}

impl FileTrackReader {
    fn new(file: File, sector_bytes: u64, data_offset: u64) -> Result<Self, DiscError> {
        let sectors = file.metadata()?.len() / sector_bytes;
        Ok(Self {
            file,
            sector_bytes,
            data_offset,
            base: 0,
            sectors,
        })
    }

    /// A track inside `path`: sector 0 at byte `base`, `sector_bytes` per sector with
    /// user data at `data_offset`, at most `sectors` sectors (`None`: to end of file).
    pub(crate) fn open_track(
        path: &Path,
        sector_bytes: u64,
        data_offset: u64,
        base: u64,
        sectors: Option<u64>,
    ) -> Result<Self, DiscError> {
        let file = open_track_file(path)?;
        let available = file.metadata()?.len().saturating_sub(base) / sector_bytes;
        Ok(Self {
            file,
            sector_bytes,
            data_offset,
            base,
            sectors: sectors.map_or(available, |s| s.min(available)),
        })
    }

    /// Opens a bare `.bin` / `.iso` / `.img`, detecting the layout from sector 0: the
    /// raw sync pattern means 2352-byte sectors (mode byte 15 picks the data offset),
    /// otherwise 2048-byte ISO sectors.
    pub fn open_auto(path: &Path) -> Result<Self, DiscError> {
        let mut file = File::open(path)?;
        let mut head = [0u8; 16];
        let raw = file.read_exact(&mut head).is_ok() && head[..12] == SYNC;
        file.seek(SeekFrom::Start(0))?;
        let mode = if !raw {
            TrackMode::Mode1Cooked
        } else if head[15] == 1 {
            TrackMode::Mode1Raw
        } else {
            TrackMode::Mode2Raw
        };
        Self::open_with_mode(file, mode)
    }

    fn open_with_mode(file: File, mode: TrackMode) -> Result<Self, DiscError> {
        let (sector_bytes, data_offset) = mode_layout(mode)?;
        Self::new(file, sector_bytes, data_offset)
    }
}

/// Sector size and user-data offset of a cue track mode.
fn mode_layout(mode: TrackMode) -> Result<(u64, u64), DiscError> {
    match mode {
        TrackMode::Mode1Raw => Ok((RAW_SECTOR_BYTES, 16)),
        TrackMode::Mode2Raw => Ok((RAW_SECTOR_BYTES, 24)),
        TrackMode::Mode1Cooked => Ok((SECTOR_DATA_BYTES as u64, 0)),
        TrackMode::Audio | TrackMode::Other => Err(DiscError::NoDataTrack),
    }
}

impl DiscReader for FileTrackReader {
    fn read_user_data(&mut self, lba: u32) -> Result<SectorData, DiscError> {
        let sector = u64::from(lba);
        if sector >= self.sectors {
            return Err(DiscError::SectorOutOfRange(lba));
        }
        let mut data = [0u8; SECTOR_DATA_BYTES];
        let offset = self.base + sector * self.sector_bytes + self.data_offset;
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(&mut data)?;
        Ok(data)
    }

    fn sector_count(&self) -> u64 {
        self.sectors
    }
}

/// Opens the first data track of an image: a `.cue` (its first data track file, in
/// the declared mode, from its `INDEX 01`), a `.chd`, a `.cso`, or a bare `.bin` / `.iso`
/// (layout auto-detected). Fails with [`DiscError::MissingFile`] when a cue's data
/// file does not exist; `.m3u` / `.pbp` are not single images (resolve them first).
pub fn open_data_track(path: &Path) -> Result<Box<dyn DiscReader>, DiscError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_m3u_ext(ext) || ext.eq_ignore_ascii_case(PBP_EXT) {
        return Err(DiscError::Format("not a single disc image"));
    }
    if is_cue_ext(ext) {
        let sheet = CueSheet::read(path)?;
        let (track, entry) = sheet.first_data_track(path)?;
        Ok(Box::new(open_cue_track(&track, &entry)?))
    } else if ext.eq_ignore_ascii_case(CHD_EXT) {
        open_chd(path)
    } else if ext.eq_ignore_ascii_case(CSO_EXT) {
        Ok(Box::new(CsoReader::open(path)?))
    } else {
        Ok(Box::new(FileTrackReader::open_auto(path)?))
    }
}

/// The data track `entry` of a cue sheet, stored in `file`, from its `INDEX 01`.
pub(crate) fn open_cue_track(file: &Path, entry: &CueTrack) -> Result<FileTrackReader, DiscError> {
    let (sector_bytes, data_offset) = mode_layout(entry.mode)?;
    let base = entry.index01.map_or(0, u64::from) * sector_bytes;
    FileTrackReader::open_track(file, sector_bytes, data_offset, base, None)
}

/// Opens a track file named by a sheet: a missing file is [`DiscError::MissingFile`].
pub(crate) fn open_track_file(path: &Path) -> Result<File, DiscError> {
    File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            DiscError::MissingFile(crate::m3u_playlist::display_name(path))
        }
        _ => DiscError::Io(e),
    })
}

#[cfg(test)]
#[path = "disc_sector_reader_tests.rs"]
mod tests;
