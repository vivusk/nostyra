//! Every track of a disc image with its absolute disc LBA, and reads by absolute LBA
//! inside one data track: what rcheevos' cdreader needs (open a track by number or a
//! special pick, `first_track_sector`, `read_sector` at an absolute sector).
//!
//! LBAs follow rcheevos' convention: track 1 starts at LBA 0 (the 150-frame lead-in
//! pregap is not counted); a GD-ROM's high-density area starts at LBA 45000.
//! Formats: `.cue` (multi-file; Redump `REM HIGH-DENSITY AREA`), `.gdi`, `.chd`
//! (CD / GD-ROM / DVD), `.cdi` (sessions), and single-track `.cso` / `.iso` / `.bin`.
//! Audio tracks are listed but cannot be opened (no reader returns raw audio).

use std::path::Path;

use crate::cdi_image::CdiImage;
use crate::chd_track_list::{chd_tracks, open_chd_track};
use crate::cso_image::CsoReader;
use crate::cue_sheet::CueSheet;
use crate::disc_sector_reader::{open_cue_track, DiscReader, FileTrackReader};
use crate::disc_track_lists::{cdi_tracks, cue_tracks, gdi_tracks, single_track};
use crate::dreamcast_ip_track::open_gdi_track;
use crate::gdi_sheet::GdiSheet;
use crate::SECTOR_DATA_BYTES;
use crate::{is_cue_ext, is_gdi_ext, is_m3u_ext, DiscError, CDI_EXT, CHD_EXT, CSO_EXT, PBP_EXT};

/// One track of a disc image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackInfo {
    /// Track number as the image numbers it (1-based).
    pub number: u32,
    /// 0-based session (`.cue`: always 0, `REM SESSION` is not read; `.gdi` / GD
    /// CHD: 1 for the high-density area).
    pub session: u16,
    /// Data track (else audio).
    pub data: bool,
    /// Absolute disc LBA of the track's sector 0 (`INDEX 01`).
    pub start_lba: u32,
    /// Sectors from `start_lba` to the next track (or the end of the image).
    pub sector_count: u64,
}

/// Which track to open (rcheevos `RC_HASH_CDTRACK_*` picks, or a number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackSelector {
    Number(u32),
    /// The first data track.
    FirstData,
    /// The last track (data or audio).
    Last,
    /// The largest data track (the first one on ties).
    Largest,
    /// The first track of the second session.
    FirstOfSecondSession,
}

impl TrackSelector {
    /// The track this selector picks from `tracks` (disc order).
    pub fn pick(self, tracks: &[TrackInfo]) -> Option<&TrackInfo> {
        match self {
            Self::Number(n) => tracks.iter().find(|t| t.number == n),
            Self::FirstData => tracks.iter().find(|t| t.data),
            Self::Last => tracks.last(),
            Self::Largest => tracks.iter().filter(|t| t.data).fold(
                None,
                |best: Option<&TrackInfo>, t| match best {
                    Some(b) if b.sector_count >= t.sector_count => Some(b),
                    _ => Some(t),
                },
            ),
            Self::FirstOfSecondSession => tracks.iter().find(|t| t.session == 1),
        }
    }
}

fn ext_of(path: &Path) -> &str {
    path.extension().and_then(|e| e.to_str()).unwrap_or("")
}

/// Every track of the image at `path`, in disc order. `.m3u` / `.pbp` are not single
/// disc images (resolve them first).
pub fn track_list(path: &Path) -> Result<Vec<TrackInfo>, DiscError> {
    let ext = ext_of(path);
    if is_m3u_ext(ext) || ext.eq_ignore_ascii_case(PBP_EXT) {
        return Err(DiscError::Format("not a single disc image"));
    }
    if is_cue_ext(ext) {
        cue_tracks(path)
    } else if is_gdi_ext(ext) {
        gdi_tracks(path)
    } else if ext.eq_ignore_ascii_case(CHD_EXT) {
        chd_tracks(path)
    } else if ext.eq_ignore_ascii_case(CDI_EXT) {
        cdi_tracks(path)
    } else {
        Ok(vec![single_track(open_single(path)?.sector_count())])
    }
}

/// An opened data track, read by absolute disc LBA.
pub struct DiscTrack {
    reader: Box<dyn DiscReader>,
    info: TrackInfo,
}

impl DiscTrack {
    /// Opens the track `selector` picks; it must be a data track.
    pub fn open(path: &Path, selector: TrackSelector) -> Result<Self, DiscError> {
        let tracks = track_list(path)?;
        let info = selector
            .pick(&tracks)
            .filter(|t| t.data)
            .cloned()
            .ok_or(DiscError::NoDataTrack)?;
        let reader = open_track_reader(path, info.number)?;
        Ok(Self { reader, info })
    }

    pub fn info(&self) -> &TrackInfo {
        &self.info
    }

    /// Absolute LBA of the track's sector 0.
    pub fn first_track_sector(&self) -> u32 {
        self.info.start_lba
    }

    /// Fills `buf` with user data (2048 bytes per sector) from absolute `lba` on,
    /// continuing into the following sectors; returns the bytes written. 0 before the
    /// track start; short at the end of the track or on a read error.
    pub fn read_user_data_abs(&mut self, lba: u32, buf: &mut [u8]) -> usize {
        let Some(mut sector) = lba.checked_sub(self.info.start_lba) else {
            return 0;
        };
        let mut done = 0;
        while done < buf.len() {
            let Ok(data) = self.reader.read_user_data(sector) else {
                break;
            };
            let n = (buf.len() - done).min(SECTOR_DATA_BYTES);
            buf[done..done + n].copy_from_slice(&data[..n]);
            done += n;
            let Some(next) = sector.checked_add(1) else {
                break;
            };
            sector = next;
        }
        done
    }
}

/// Single-track images: CSO, else a bare `.iso` / `.bin` (layout auto-detected).
fn open_single(path: &Path) -> Result<Box<dyn DiscReader>, DiscError> {
    if ext_of(path).eq_ignore_ascii_case(CSO_EXT) {
        Ok(Box::new(CsoReader::open(path)?))
    } else {
        Ok(Box::new(FileTrackReader::open_auto(path)?))
    }
}

/// Sector reader for data track `number` (track-relative sectors).
fn open_track_reader(path: &Path, number: u32) -> Result<Box<dyn DiscReader>, DiscError> {
    let ext = ext_of(path);
    if is_cue_ext(ext) {
        let sheet = CueSheet::read(path)?;
        let (file, entry) = sheet
            .data_tracks(path)
            .into_iter()
            .find(|(_, t)| t.number == number)
            .ok_or(DiscError::NoDataTrack)?;
        return Ok(Box::new(open_cue_track(&file, &entry)?));
    }
    if is_gdi_ext(ext) {
        let sheet = GdiSheet::read(path)?;
        let (track, file) = sheet
            .tracks
            .iter()
            .zip(sheet.referenced_paths(path))
            .find(|(t, _)| t.number == number)
            .ok_or(DiscError::NoDataTrack)?;
        return Ok(Box::new(open_gdi_track(&file, track.sector_size)?));
    }
    if ext.eq_ignore_ascii_case(CHD_EXT) {
        return open_chd_track(path, number);
    }
    if ext.eq_ignore_ascii_case(CDI_EXT) {
        let image = CdiImage::read(path)?;
        let index = number.checked_sub(1).ok_or(DiscError::NoDataTrack)? as usize;
        let track = image.tracks.get(index).ok_or(DiscError::NoDataTrack)?;
        return Ok(Box::new(CdiImage::open_track(path, track)?));
    }
    match number {
        1 => open_single(path),
        _ => Err(DiscError::NoDataTrack),
    }
}

#[cfg(test)]
#[path = "disc_track_access_tests.rs"]
mod tests;
