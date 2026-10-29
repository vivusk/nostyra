//! CHD track list with absolute LBAs (see `disc_track_access`).
//!
//! CD: each track's `INDEX 01` LBA = the previous track's end + this track's pregap
//! (stored or not: it occupies disc space either way), normalised so track 1 starts
//! at 0. GD-ROM: same for the single-density tracks 1–2; track 3 opens the
//! high-density area at LBA 45000. DVD (no track
//! metadata, PSP UMD): one data track at LBA 0. Not yet checked against a real
//! multi-track CD CHD (only single-track PS1 and GD-ROM images are verified).

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use chd::metadata::Metadata;
use chd::Chd;

use crate::chd_dvd_reader::open_chd;
use crate::chd_gd_tracks::{is_data, track_field, track_table, ChdTrackEntry};
use crate::chd_track_reader::ChdTrackReader;
use crate::disc_sector_reader::DiscReader;
use crate::disc_track_access::TrackInfo;
use crate::disc_track_lists::single_track;
use crate::gdi_sheet::HIGH_DENSITY_LBA;
use crate::DiscError;

fn entries(path: &Path) -> Result<Vec<ChdTrackEntry>, DiscError> {
    let mut chd = Chd::open(BufReader::new(File::open(path)?), None)?;
    let metas: Vec<Metadata> = chd.metadata_refs().try_into()?;
    Ok(track_table(&metas))
}

fn number(entry: &ChdTrackEntry, key: &str) -> u64 {
    track_field(&entry.text, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

pub(crate) fn chd_tracks(path: &Path) -> Result<Vec<TrackInfo>, DiscError> {
    let tracks = entries(path)?;
    if tracks.is_empty() {
        return Ok(vec![single_track(open_chd(path)?.sector_count())]);
    }
    let gd = tracks.iter().any(|t| t.gd);
    let mut out = Vec::new();
    let mut next = 0u64;
    for track in &tracks {
        let pregap = number(track, "PREGAP");
        let stored = track_field(&track.text, "PGTYPE").is_some_and(|p| p.starts_with('V'));
        let sectors = number(track, "FRAMES").saturating_sub(if stored { pregap } else { 0 });
        let mut start = next + pregap;
        if gd && track.number == 3 {
            start = start.max(u64::from(HIGH_DENSITY_LBA));
        }
        next = start + sectors;
        out.push(TrackInfo {
            number: track.number,
            session: u16::from(gd && track.number >= 3),
            data: is_data(track),
            start_lba: u32::try_from(start)
                .map_err(|_| DiscError::Format("CHD track LBA out of range"))?,
            sector_count: sectors,
        });
    }
    // Track 1's lead-in pregap is not counted (the GD high-density area keeps 45000).
    let base = out.first().map_or(0, |t| t.start_lba);
    out.iter_mut()
        .filter(|t| t.session == 0)
        .for_each(|t| t.start_lba -= base);
    Ok(out)
}

/// Data track `number` (track-relative sectors); a DVD image's only track is 1.
pub(crate) fn open_chd_track(path: &Path, number: u32) -> Result<Box<dyn DiscReader>, DiscError> {
    if entries(path)?.is_empty() {
        return match number {
            1 => open_chd(path),
            _ => Err(DiscError::NoDataTrack),
        };
    }
    Ok(Box::new(ChdTrackReader::open_track(path, |tracks| {
        tracks.iter().find(|t| t.number == number)
    })?))
}
