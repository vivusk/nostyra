//! Which file a disc game's identity hashes (scan and runner share this rule, so a
//! game keeps one content id / save folder):
//!
//! - `.m3u`: the rule of its first disc.
//! - PlayStation `.cue`: the first data track file (what the Redump PS1 DAT hashes).
//! - Dreamcast `.gdi` / `.cue`: the largest data track file (the one rom the Redump
//!   Dreamcast DAT lists per game; ties → the last such track).
//! - Anything else (`.chd`, `.cdi`, `.pbp`, `.cso`, a bare image, every PSP image):
//!   the file itself (the Redump PSP DAT hashes the whole `.iso`).
//!
//! The signature is needed: `.cue`, `.chd` and `.m3u` are shared between systems.

use std::path::{Path, PathBuf};

use system_catalog::DiscSignature;

use crate::cue_sheet::CueSheet;
use crate::gdi_sheet::GdiSheet;
use crate::m3u_playlist::playlist_entries;
use crate::{is_cue_ext, is_gdi_ext, is_m3u_ext, DiscError};

/// The file whose bytes identify the disc game at `path`.
pub fn identity_source(path: &Path, signature: DiscSignature) -> Result<PathBuf, DiscError> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_m3u_ext(ext) {
        let first = playlist_entries(path)?
            .into_iter()
            .next()
            .ok_or(DiscError::EmptyPlaylist)?;
        // Nested playlists are rejected by the parser: this recursion is depth 1.
        return identity_source(&first, signature);
    }
    match signature {
        DiscSignature::PlayStation if is_cue_ext(ext) => {
            Ok(CueSheet::read(path)?.first_data_track(path)?.0)
        }
        DiscSignature::Dreamcast if is_cue_ext(ext) => {
            let files = CueSheet::read(path)?.data_tracks(path);
            largest(files.into_iter().map(|(file, _)| file))
        }
        DiscSignature::Dreamcast if is_gdi_ext(ext) => {
            let sheet = GdiSheet::read(path)?;
            let files = sheet.referenced_paths(path).into_iter();
            largest(
                files
                    .zip(&sheet.tracks)
                    .filter(|(_, t)| t.data)
                    .map(|(f, _)| f),
            )
        }
        _ => Ok(path.to_path_buf()),
    }
}

/// The largest existing file (ties → the later one); a missing track file is an error.
fn largest(files: impl Iterator<Item = PathBuf>) -> Result<PathBuf, DiscError> {
    let mut best: Option<(u64, PathBuf)> = None;
    for file in files {
        let len = std::fs::metadata(&file)
            .map_err(|_| DiscError::MissingFile(crate::m3u_playlist::display_name(&file)))?
            .len();
        if best.as_ref().is_none_or(|(size, _)| len >= *size) {
            best = Some((len, file));
        }
    }
    best.map(|(_, file)| file).ok_or(DiscError::NoDataTrack)
}

#[cfg(test)]
#[path = "disc_identity_track_tests.rs"]
mod tests;
