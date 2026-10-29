//! Track lists with absolute LBAs for the sheet formats and DiscJuggler images (see
//! `disc_track_access`; CHD lives in `chd_track_list`).

use std::path::Path;

use crate::cdi_image::CdiImage;
use crate::cue_sheet::{CueSheet, TrackArea, TrackMode};
use crate::disc_sector_reader::RAW_SECTOR_BYTES;
use crate::disc_track_access::TrackInfo;
use crate::gdi_sheet::{GdiSheet, HIGH_DENSITY_LBA};
use crate::{DiscError, SECTOR_DATA_BYTES};

/// Cue: files lie end to end on the disc, a track's LBA = its file's first LBA + its
/// `INDEX 01` (as rcheevos' own cue reader computes it). The first file of the
/// high-density area (Redump GD-ROM) starts at LBA 45000.
pub(crate) fn cue_tracks(path: &Path) -> Result<Vec<TrackInfo>, DiscError> {
    let sheet = CueSheet::read(path)?;
    let mut out = Vec::new();
    let mut file_lba = 0u64;
    let mut in_high = false;
    for (file, file_path) in sheet.files.iter().zip(sheet.referenced_paths(path)) {
        let Some(first) = file.tracks.first() else {
            continue;
        };
        if first.area == TrackArea::High && !in_high {
            in_high = true;
            file_lba = file_lba.max(u64::from(HIGH_DENSITY_LBA));
        }
        let bytes = std::fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
        let file_sectors = bytes / stored_sector_bytes(first.mode);
        for (i, track) in file.tracks.iter().enumerate() {
            let start = u64::from(track.index01.unwrap_or(0));
            let end = file
                .tracks
                .get(i + 1)
                .and_then(|next| next.index01)
                .map_or(file_sectors, u64::from);
            out.push(TrackInfo {
                number: track.number,
                session: 0,
                data: track.mode.is_data(),
                start_lba: lba(file_lba + start)?,
                sector_count: end.saturating_sub(start),
            });
        }
        file_lba += file_sectors;
    }
    Ok(out)
}

fn stored_sector_bytes(mode: TrackMode) -> u64 {
    match mode {
        TrackMode::Mode1Cooked => SECTOR_DATA_BYTES as u64,
        _ => RAW_SECTOR_BYTES,
    }
}

fn lba(value: u64) -> Result<u32, DiscError> {
    u32::try_from(value).map_err(|_| DiscError::Format("track LBA out of range"))
}

/// Gdi: LBAs as written in the sheet; the high-density area is session 1.
pub(crate) fn gdi_tracks(path: &Path) -> Result<Vec<TrackInfo>, DiscError> {
    let sheet = GdiSheet::read(path)?;
    Ok(sheet
        .tracks
        .iter()
        .zip(sheet.referenced_paths(path))
        .map(|(track, file)| {
            let bytes = std::fs::metadata(file).map(|m| m.len()).unwrap_or(0);
            TrackInfo {
                number: track.number,
                session: u16::from(track.lba >= HIGH_DENSITY_LBA),
                data: track.data,
                start_lba: track.lba,
                sector_count: bytes / u64::from(track.sector_size),
            }
        })
        .collect())
}

/// DiscJuggler: LBAs and sessions from the image's table; tracks numbered 1.. in
/// table order.
pub(crate) fn cdi_tracks(path: &Path) -> Result<Vec<TrackInfo>, DiscError> {
    let image = CdiImage::read(path)?;
    Ok(image
        .tracks
        .iter()
        .zip(1u32..)
        .map(|(track, number)| TrackInfo {
            number,
            session: track.session,
            data: track.is_data(),
            start_lba: track.lba,
            sector_count: u64::from(track.length),
        })
        .collect())
}

/// A single-track image (`.iso`, `.bin`, `.cso`, DVD CHD): data track 1 at LBA 0.
pub(crate) fn single_track(sector_count: u64) -> TrackInfo {
    TrackInfo {
        number: 1,
        session: 0,
        data: true,
        start_lba: 0,
        sector_count,
    }
}
