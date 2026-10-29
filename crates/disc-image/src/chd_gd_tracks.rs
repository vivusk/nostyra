//! Track table of a CD or GD-ROM CHD from its metadata: `CHT2`/`CHTR` (CD) and
//! `CHGD`/`CHGT` (GD-ROM, written by `chdman createcd` from a `.gdi`). Tracks are
//! stored back to back, each padded to a multiple of 4 frames (MAME/libchdr rule), so
//! a track's first frame in the image is the padded sum of the tracks before it.

use chd::metadata::{KnownMetadata, Metadata};

/// Tracks are padded to this many frames inside a CHD.
const TRACK_PADDING_FRAMES: u64 = 4;
/// `CHGT` (old GD-ROM track metadata) and `CHGD` (current), as big-endian tags.
const GD_TRACK_OLD: u32 = u32::from_be_bytes(*b"CHGT");
const GD_TRACK: u32 = u32::from_be_bytes(*b"CHGD");

/// One track's metadata and where its frames start in the image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChdTrackEntry {
    pub number: u32,
    /// The metadata text (`TRACK:3 TYPE:MODE1_RAW … FRAMES:… PREGAP:…`).
    pub text: String,
    /// Image frame of the track's first stored frame (pregap included when stored).
    pub first_frame: u64,
    /// Whether the metadata is a GD-ROM tag.
    pub gd: bool,
}

/// Every CD/GD track of the image, ordered by track number.
pub(crate) fn track_table(metas: &[Metadata]) -> Vec<ChdTrackEntry> {
    let mut tracks: Vec<(u32, String, bool, u64)> = metas
        .iter()
        .filter_map(|m| {
            let gd = m.metatag == GD_TRACK || m.metatag == GD_TRACK_OLD;
            let cd = m.metatag == KnownMetadata::CdRomTrack2 as u32
                || m.metatag == KnownMetadata::CdRomTrack as u32;
            if !gd && !cd {
                return None;
            }
            let text = String::from_utf8_lossy(&m.value)
                .trim_end_matches('\0')
                .to_owned();
            let number = track_field(&text, "TRACK")?.parse().ok()?;
            let frames = track_field(&text, "FRAMES")?.parse().ok()?;
            Some((number, text, gd, frames))
        })
        .collect();
    tracks.sort_by_key(|t| t.0);
    let mut next = 0u64;
    tracks
        .into_iter()
        .map(|(number, text, gd, frames)| {
            let first_frame = next;
            next += frames.div_ceil(TRACK_PADDING_FRAMES) * TRACK_PADDING_FRAMES;
            ChdTrackEntry {
                number,
                text,
                first_frame,
                gd,
            }
        })
        .collect()
}

/// `KEY:value` from a metadata text.
pub(crate) fn track_field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.split_whitespace()
        .find_map(|word| word.strip_prefix(key)?.strip_prefix(':'))
}

/// Whether the track carries data (any type but `AUDIO`).
pub(crate) fn is_data(entry: &ChdTrackEntry) -> bool {
    track_field(&entry.text, "TYPE").is_some_and(|t| t != "AUDIO")
}

/// IP.BIN track of a Dreamcast CHD: GD-ROM → the first data track after track 2
/// (high-density area); CD (MIL-CD) → the last data track's session start, i.e. the
/// first data track after the last audio track, else the first data track.
pub(crate) fn boot_track(tracks: &[ChdTrackEntry]) -> Option<&ChdTrackEntry> {
    if tracks.iter().any(|t| t.gd) {
        return tracks.iter().find(|t| t.number >= 3 && is_data(t));
    }
    let after_audio = tracks
        .iter()
        .rposition(|t| !is_data(t))
        .and_then(|at| tracks[at + 1..].iter().find(|t| is_data(t)));
    after_audio.or_else(|| tracks.iter().find(|t| is_data(t)))
}
