//! `.gdi` sheet (Dreamcast GD-ROM dumps, TOSEC layout): a track count line, then one
//! line per track: `number lba ctrl sector_size file offset`. `ctrl` 4 = data, 0 =
//! audio; sector size 2352 (raw) or 2048; file names may be quoted (spaces) and
//! resolve against the sheet's folder. Track 3 (data, LBA 45000) starts the
//! high-density area holding IP.BIN; the parser does not require it (the probe does).

use std::path::{Path, PathBuf};

use crate::sheet_text::read_sheet_text;
use crate::DiscError;

/// First LBA of a GD-ROM's high-density area.
pub const HIGH_DENSITY_LBA: u32 = 45_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GdiTrack {
    pub number: u32,
    /// Disc LBA of the track's first sector.
    pub lba: u32,
    /// `ctrl` bit 2: a data track (else audio).
    pub data: bool,
    /// Bytes per stored sector: 2352 or 2048.
    pub sector_size: u32,
    /// File name exactly as written (quotes removed).
    pub file: String,
    /// The offset column (0 in every known dump; kept for completeness).
    pub offset: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GdiSheet {
    pub tracks: Vec<GdiTrack>,
}

const BAD_GDI: &str = "not a valid GDI sheet";

impl GdiSheet {
    /// Parses gdi text. Strict (unlike cue): a malformed line rejects the sheet.
    pub fn parse(text: &str) -> Result<Self, DiscError> {
        let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        let count: usize = lines
            .next()
            .and_then(|l| l.parse().ok())
            .filter(|n| (1..=99).contains(n))
            .ok_or(DiscError::Format(BAD_GDI))?;
        let tracks = lines
            .map(parse_track)
            .collect::<Option<Vec<_>>>()
            .ok_or(DiscError::Format(BAD_GDI))?;
        if tracks.len() != count {
            return Err(DiscError::Format(
                "GDI track count does not match its lines",
            ));
        }
        Ok(Self { tracks })
    }

    /// Reads and parses a gdi file.
    pub fn read(gdi_path: &Path) -> Result<Self, DiscError> {
        Self::parse(&read_sheet_text(gdi_path)?)
    }

    /// Every track file, resolved against `gdi_path`'s folder (sheet order).
    pub fn referenced_paths(&self, gdi_path: &Path) -> Vec<PathBuf> {
        let dir = gdi_path.parent().unwrap_or(Path::new(""));
        self.tracks.iter().map(|t| dir.join(&t.file)).collect()
    }

    /// Names (as written) of track files that do not exist.
    pub fn missing_files(&self, gdi_path: &Path) -> Vec<String> {
        self.tracks
            .iter()
            .zip(self.referenced_paths(gdi_path))
            .filter(|(_, path)| !path.is_file())
            .map(|(track, _)| track.file.clone())
            .collect()
    }

    /// The first data track of the high-density area (IP.BIN in its sector 0).
    pub fn high_density_data_track(&self) -> Option<&GdiTrack> {
        self.tracks
            .iter()
            .find(|t| t.data && t.lba >= HIGH_DENSITY_LBA)
    }
}

/// `3 45000 4 2352 "Game (Track 3).bin" 0` → a track; `None` when malformed.
fn parse_track(line: &str) -> Option<GdiTrack> {
    let words = tokens(line)?;
    let [number, lba, ctrl, size, file, rest @ ..] = words.as_slice() else {
        return None;
    };
    let ctrl: u32 = ctrl.parse().ok()?;
    let offset = match rest {
        [] => 0,
        [offset] => offset.parse().ok()?,
        _ => return None,
    };
    Some(GdiTrack {
        number: number.parse().ok()?,
        lba: lba.parse().ok()?,
        data: ctrl & 4 != 0,
        sector_size: size.parse().ok().filter(|s| matches!(s, 2048 | 2352))?,
        file: (*file).to_owned(),
        offset,
    })
}

/// Whitespace-separated words; `"…"` is one word without its quotes. `None` on an
/// unclosed quote or an empty quoted name.
fn tokens(line: &str) -> Option<Vec<&str>> {
    let mut out = Vec::new();
    let mut rest = line.trim_start();
    while !rest.is_empty() {
        let (word, tail) = match rest.strip_prefix('"') {
            Some(quoted) => quoted.split_once('"').filter(|(w, _)| !w.is_empty())?,
            None => rest.split_once(char::is_whitespace).unwrap_or((rest, "")),
        };
        out.push(word);
        rest = tail.trim_start();
    }
    Some(out)
}

#[cfg(test)]
#[path = "gdi_sheet_tests.rs"]
mod tests;
