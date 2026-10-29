//! `.cue` sheet parser: the `FILE` / `TRACK` / `INDEX 01` lines a disc image needs,
//! plus Redump's GD-ROM `REM SINGLE-DENSITY AREA` / `REM HIGH-DENSITY AREA` markers.
//! Keywords are case-insensitive; other `REM`s, `CATALOG`, `PREGAP`, … are ignored.
//! File names resolve against the cue's folder (absolute names are kept as written).

use std::path::{Path, PathBuf};

pub use crate::cue_track_mode::TrackMode;
use crate::sheet_text::read_sheet_text;
use crate::DiscError;

/// GD-ROM area a track belongs to (Redump `REM … DENSITY AREA`); every CD track is
/// `Single`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TrackArea {
    #[default]
    Single,
    /// High-density area (LBA 45000+): its first data track holds IP.BIN.
    High,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueTrack {
    pub number: u32,
    pub mode: TrackMode,
    pub area: TrackArea,
    /// `INDEX 01` position inside the file, in frames (75 per second).
    pub index01: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueFile {
    /// File name exactly as written in the sheet.
    pub name: String,
    pub tracks: Vec<CueTrack>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CueSheet {
    pub files: Vec<CueFile>,
}

impl CueSheet {
    /// Parses cue text. Lenient: unknown or malformed lines are skipped.
    pub fn parse(text: &str) -> Self {
        let mut sheet = Self::default();
        let mut area = TrackArea::Single;
        for line in text.lines().map(str::trim) {
            let (keyword, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let rest = rest.trim();
            match keyword.to_ascii_uppercase().as_str() {
                "FILE" => sheet.files.push(CueFile {
                    name: file_name(rest).to_owned(),
                    tracks: Vec::new(),
                }),
                "TRACK" => {
                    let mut words = rest.split_whitespace();
                    let number = words.next().and_then(|n| n.parse().ok());
                    let mode = words.next().map(TrackMode::parse);
                    if let (Some(file), Some(number), Some(mode)) =
                        (sheet.files.last_mut(), number, mode)
                    {
                        file.tracks.push(CueTrack {
                            number,
                            mode,
                            area,
                            index01: None,
                        });
                    }
                }
                "INDEX" => {
                    let mut words = rest.split_whitespace();
                    if words.next().and_then(|n| n.parse::<u32>().ok()) == Some(1) {
                        let track = sheet.files.last_mut().and_then(|f| f.tracks.last_mut());
                        if let (Some(track), Some(pos)) = (track, words.next().and_then(msf)) {
                            track.index01 = Some(pos);
                        }
                    }
                }
                "REM" => {
                    let rest = rest.to_ascii_uppercase();
                    if rest.starts_with("HIGH-DENSITY AREA") {
                        area = TrackArea::High;
                    } else if rest.starts_with("SINGLE-DENSITY AREA") {
                        area = TrackArea::Single;
                    }
                }
                _ => {}
            }
        }
        sheet
    }

    /// Reads and parses a cue file.
    pub fn read(cue_path: &Path) -> Result<Self, DiscError> {
        Ok(Self::parse(&read_sheet_text(cue_path)?))
    }

    /// Every referenced file, resolved against `cue_path`'s folder (sheet order).
    pub fn referenced_paths(&self, cue_path: &Path) -> Vec<PathBuf> {
        let dir = cue_path.parent().unwrap_or(Path::new(""));
        self.files.iter().map(|f| dir.join(&f.name)).collect()
    }

    /// Names (as written) of referenced files that do not exist.
    pub fn missing_files(&self, cue_path: &Path) -> Vec<String> {
        self.files
            .iter()
            .zip(self.referenced_paths(cue_path))
            .filter(|(_, path)| !path.is_file())
            .map(|(file, _)| file.name.clone())
            .collect()
    }

    /// File and track entry of the first data track (PlayStation discs: track 1).
    pub fn first_data_track(&self, cue_path: &Path) -> Result<(PathBuf, CueTrack), DiscError> {
        self.find_data_track(cue_path, |_| true)
    }

    /// The first data track of the high-density area when the sheet has one (GD-ROM:
    /// IP.BIN in its sector 0), else the first data track.
    pub fn boot_data_track(&self, cue_path: &Path) -> Result<(PathBuf, CueTrack), DiscError> {
        self.find_data_track(cue_path, |t| t.area == TrackArea::High)
            .or_else(|_| self.first_data_track(cue_path))
    }

    /// Every data track with its file (sheet order).
    pub fn data_tracks(&self, cue_path: &Path) -> Vec<(PathBuf, CueTrack)> {
        self.files
            .iter()
            .zip(self.referenced_paths(cue_path))
            .flat_map(|(file, path)| {
                file.tracks
                    .iter()
                    .filter(|t| t.mode.is_data())
                    .map(move |t| (path.clone(), t.clone()))
            })
            .collect()
    }

    fn find_data_track(
        &self,
        cue_path: &Path,
        pick: impl Fn(&CueTrack) -> bool,
    ) -> Result<(PathBuf, CueTrack), DiscError> {
        self.data_tracks(cue_path)
            .into_iter()
            .find(|(_, t)| pick(t))
            .ok_or(DiscError::NoDataTrack)
    }
}

/// `"Game (Track 1).bin" BINARY` → `Game (Track 1).bin`; unquoted names run up to the
/// last word (the file type).
fn file_name(rest: &str) -> &str {
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted.split('"').next().unwrap_or("");
    }
    match rest.rsplit_once(char::is_whitespace) {
        Some((name, _file_type)) => name.trim(),
        None => rest,
    }
}

/// `mm:ss:ff` → frames.
fn msf(text: &str) -> Option<u32> {
    let mut parts = text.split(':').map(|p| p.parse::<u32>().ok());
    let (m, s, f) = (parts.next()??, parts.next()??, parts.next()??);
    (parts.next().is_none() && s < 60 && f < 75).then(|| (m * 60 + s) * 75 + f)
}

#[cfg(test)]
#[path = "cue_sheet_tests.rs"]
mod tests;
