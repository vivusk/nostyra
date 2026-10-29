//! `.m3u` multi-disc playlist: one disc image per line, `#` lines and blanks skipped,
//! relative entries resolve against the playlist's folder (absolute ones are kept).
//! PCSX ReARMed reads the same format and accepts at most 8 discs.

use std::path::{Path, PathBuf};

use crate::sheet_text::read_sheet_text;
use crate::{is_m3u_ext, DiscError, MAX_PLAYLIST_ENTRIES};

/// Entries of `text`, resolved against `dir`. Errors on 0 or more than 8 entries, or
/// an entry that is itself an m3u (the core does not nest playlists).
pub fn parse_playlist(text: &str, dir: &Path) -> Result<Vec<PathBuf>, DiscError> {
    let entries: Vec<PathBuf> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| dir.join(line))
        .collect();
    if entries.is_empty() {
        return Err(DiscError::EmptyPlaylist);
    }
    if entries.len() > MAX_PLAYLIST_ENTRIES {
        return Err(DiscError::TooManyDiscs(entries.len()));
    }
    if let Some(nested) = entries.iter().find(|e| ext_of(e).is_some_and(is_m3u_ext)) {
        return Err(DiscError::NestedPlaylist(display_name(nested)));
    }
    Ok(entries)
}

/// Reads and parses an m3u file.
pub fn playlist_entries(m3u_path: &Path) -> Result<Vec<PathBuf>, DiscError> {
    let dir = m3u_path.parent().unwrap_or(Path::new(""));
    parse_playlist(&read_sheet_text(m3u_path)?, dir)
}

fn ext_of(path: &Path) -> Option<&str> {
    path.extension().and_then(|e| e.to_str())
}

pub(crate) fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
#[path = "m3u_playlist_tests.rs"]
mod tests;
