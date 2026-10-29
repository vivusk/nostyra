//! Error type for every disc-image read. Messages are user-facing (scan problems
//! dialog, runner invalid-ROM text), so they name the file or entry at fault.

use thiserror::Error;

/// Why a disc image (or its cue / m3u sheet) could not be read.
#[derive(Debug, Error)]
pub enum DiscError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// A cue `FILE` line or an m3u entry names a file that does not exist.
    #[error("track file '{0}' not found")]
    MissingFile(String),
    #[error("cue sheet has no data track")]
    NoDataTrack,
    #[error("playlist lists no discs")]
    EmptyPlaylist,
    #[error("playlist lists {0} discs (at most {max})", max = crate::MAX_PLAYLIST_ENTRIES)]
    TooManyDiscs(usize),
    /// Sheets nest deeper than one level (an m3u entry that is itself an m3u).
    #[error("playlist entry '{0}' is itself a playlist")]
    NestedPlaylist(String),
    #[error("sector {0} is past the end of the image")]
    SectorOutOfRange(u32),
    /// Structurally not what was expected (no ISO9660 volume, bad PBP header, …).
    #[error("{0}")]
    Format(&'static str),
    /// An ISO9660 volume that is not a PSP game (UMD Video, a PC DVD, …).
    #[error("{0}")]
    NotPsp(&'static str),
    #[error("CHD image: {0}")]
    Chd(#[from] chd::Error),
}
