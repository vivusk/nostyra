//! Multi-file disc games, after the walk of one library root: a `.cue`, `.gdi` or
//! `.m3u` is the game; every file a sheet references (tracks, the discs of a playlist and
//! their tracks, in any folder) is hidden, and so are walk failures on hidden files
//! (audio tracks fail the PlayStation probe). Broken sheets were already reported
//! once by the classifier; here they only hide what clearly belongs to them:
//! a same-stem `.bin` beside a cue, and bins whose stem starts with the cue's stem
//! or a missing `FILE`'s stem (a renamed track 1). A game's fingerprint covers its
//! files: size = sum, mtime = newest.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use disc_image::{
    is_cue_ext, is_gdi_ext, is_m3u_ext, is_sheet_ext, playlist_entries, CueSheet, GdiSheet,
};
use system_catalog::{Media, SystemSpec};

use crate::folder_scanner::DiscoveredFiles;

/// What one sheet references.
struct SheetRefs {
    sheet: PathBuf,
    /// Existing referenced files (normalised), playlist discs' tracks included.
    existing: Vec<PathBuf>,
    /// Stems that hide sibling bins (cue only): own stem + missing `FILE` stems,
    /// when the cue is broken.
    broken_stems: Vec<String>,
}

/// Groups the disc games in `found` (one walk of one library root).
pub(crate) fn group_disc_files(found: &mut DiscoveredFiles, systems: &[&'static SystemSpec]) {
    let disc_ext = |path: &Path| {
        let ext = extension(path);
        systems
            .iter()
            .any(|s| s.media == Media::Disc && s.has_extension(ext))
    };
    let sheet_paths = found
        .files
        .iter()
        .map(|f| PathBuf::from(&f.path))
        .chain(found.failures.iter().map(|f| f.path.clone()))
        .filter(|path| disc_ext(path) && is_sheet_ext(extension(path)));
    let sheets: Vec<SheetRefs> = sheet_paths.map(|path| sheet_refs(&path)).collect();
    if sheets.is_empty() {
        return;
    }
    let referenced: HashSet<PathBuf> = sheets.iter().flat_map(|s| s.existing.clone()).collect();
    // Referenced files (cues of a playlist included) and, for non-sheets, the cue stem
    // rules. Failures on such paths go too (they belong to a listed or reported game).
    // The stem rules guess, so they never hide a file classified as a cartridge (a
    // Mega Drive `Sonic.bin` beside an unrelated `Sonic.cue`); walk failures keep
    // the extension rule.
    let is_hidden = |path: &Path, disc_classified: bool| {
        disc_ext(path)
            && (referenced.contains(&normalise(path))
                || (disc_classified
                    && !is_sheet_ext(extension(path))
                    && hidden_by_cue_stem(path, &sheets)))
    };
    found
        .files
        .retain(|f| !is_hidden(Path::new(&f.path), f.system.media == Media::Disc));
    found.failures.retain(|f| !is_hidden(&f.path, true));
    for file in found.files.iter_mut() {
        let Some(refs) = sheets.iter().find(|s| s.sheet == Path::new(&file.path)) else {
            continue;
        };
        for path in &refs.existing {
            if let Ok(meta) = std::fs::metadata(path) {
                file.size += meta.len();
                file.mtime_ns = file.mtime_ns.max(mtime_ns(&meta));
            }
        }
    }
}

fn sheet_refs(sheet: &Path) -> SheetRefs {
    let mut refs = SheetRefs {
        sheet: sheet.to_path_buf(),
        existing: Vec::new(),
        broken_stems: Vec::new(),
    };
    if is_m3u_ext(extension(sheet)) {
        for disc in playlist_entries(sheet).unwrap_or_default() {
            if disc.is_file() {
                if is_sheet_ext(extension(&disc)) {
                    refs.existing.extend(sheet_refs(&disc).existing);
                }
                refs.existing.push(normalise(&disc));
            }
        }
        return refs;
    }
    if is_gdi_ext(extension(sheet)) {
        // A broken gdi was reported by the classifier; its existing tracks still hide.
        if let Ok(gdi) = GdiSheet::read(sheet) {
            refs.existing = existing(&gdi.referenced_paths(sheet));
        }
        return refs;
    }
    let Ok(cue) = CueSheet::read(sheet) else {
        return refs;
    };
    let missing = cue.missing_files(sheet);
    refs.existing = existing(&cue.referenced_paths(sheet));
    if !missing.is_empty() {
        refs.broken_stems = missing.iter().map(|name| stem(Path::new(name))).collect();
        refs.broken_stems.push(stem(sheet));
    }
    refs
}

/// The referenced files that exist, normalised.
fn existing(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| path.is_file())
        .map(|path| normalise(path))
        .collect()
}

/// Rule (b) / (c): a bin beside a cue with the cue's stem, or beside a broken cue
/// with a stem starting with one of its broken stems.
fn hidden_by_cue_stem(path: &Path, sheets: &[SheetRefs]) -> bool {
    let own = stem(path);
    sheets
        .iter()
        .filter(|s| is_cue_ext(extension(&s.sheet)) && s.sheet.parent() == path.parent())
        .any(|cue| {
            own == stem(&cue.sheet) || cue.broken_stems.iter().any(|b| own.starts_with(b.as_str()))
        })
}

/// Lower-cased file stem (Windows names are case-insensitive; a renamed track keeps
/// its prefix in any case).
fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn extension(path: &Path) -> &str {
    path.extension().and_then(|e| e.to_str()).unwrap_or("")
}

/// Canonical form, so `a/../b.bin`, `B.BIN` and absolute m3u entries compare equal.
fn normalise(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn mtime_ns(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| {
            i64::try_from(since.as_nanos()).unwrap_or(i64::MAX)
        })
}
