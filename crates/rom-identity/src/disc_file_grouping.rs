//! Multi-file disc games, after the walk of one library root: a `.cue`, `.gdi` or
//! `.m3u` is the game; every file a sheet references (tracks, the discs of a playlist and
//! their tracks, in any folder) is hidden, and so are walk failures on hidden files
//! (audio tracks fail the PlayStation probe). Broken sheets were already reported
//! once by the classifier; here they only hide what clearly belongs to them:
//! a same-stem `.bin` beside a cue, and bins whose stem starts with the cue's stem
//! or a missing `FILE`'s stem (a renamed track 1). A game's fingerprint covers its
//! files ([`DiscGrouping::members`]: the caller sums sizes, keeps the newest mtime).
//!
//! Pure over the content source (sheets, `is_file`, `canonical`), so the desktop
//! scanner (OS files) and the web edition (registered File System Access files)
//! group the same way.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use content_file_source::{canonical, is_file};
use disc_image::{
    is_cue_ext, is_gdi_ext, is_m3u_ext, is_sheet_ext, playlist_entries, CueSheet, GdiSheet,
};

/// One classified file of the walk.
#[derive(Debug, Clone, Copy)]
pub struct DiscGroupCandidate<'a> {
    pub path: &'a Path,
    /// Classified as a disc system's file (the stem rules never hide cartridges).
    pub disc_classified: bool,
}

/// What grouping decided, by index into the caller's lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscGrouping {
    /// Files that belong to another entry (tracks, a playlist's discs): not games.
    pub hidden_files: Vec<usize>,
    /// Walk failures hidden the same way.
    pub hidden_failures: Vec<usize>,
    /// Per visible sheet (index into the files): its existing referenced files
    /// (canonical paths), which make up the game's fingerprint.
    pub members: Vec<(usize, Vec<PathBuf>)>,
}

/// What one sheet references.
struct SheetRefs {
    sheet: PathBuf,
    /// Existing referenced files (canonical), playlist discs' tracks included.
    existing: Vec<PathBuf>,
    /// Stems that hide sibling bins (cue only): own stem + missing `FILE` stems,
    /// when the cue is broken.
    broken_stems: Vec<String>,
}

/// Groups the disc games among `files` and `failures` (one walk of one library root);
/// `is_disc_ext` = some disc system of the scan uses this extension.
pub fn group_disc_files(
    files: &[DiscGroupCandidate<'_>],
    failures: &[&Path],
    is_disc_ext: &dyn Fn(&str) -> bool,
) -> DiscGrouping {
    let disc_ext = |path: &Path| is_disc_ext(extension(path));
    let sheets: Vec<SheetRefs> = files
        .iter()
        .map(|f| f.path)
        .chain(failures.iter().copied())
        .filter(|path| disc_ext(path) && is_sheet_ext(extension(path)))
        .map(sheet_refs)
        .collect();
    let mut grouping = DiscGrouping::default();
    if sheets.is_empty() {
        return grouping;
    }
    let referenced: HashSet<PathBuf> = sheets.iter().flat_map(|s| s.existing.clone()).collect();
    // Referenced files (cues of a playlist included) and, for non-sheets, the cue stem
    // rules. Failures on such paths go too (they belong to a listed or reported game).
    // The stem rules guess, so they never hide a file classified as a cartridge (a
    // Mega Drive `Sonic.bin` beside an unrelated `Sonic.cue`); walk failures keep
    // the extension rule.
    let is_hidden = |path: &Path, disc_classified: bool| {
        disc_ext(path)
            && (referenced.contains(&canonical(path))
                || (disc_classified
                    && !is_sheet_ext(extension(path))
                    && hidden_by_cue_stem(path, &sheets)))
    };
    for (index, file) in files.iter().enumerate() {
        if is_hidden(file.path, file.disc_classified) {
            grouping.hidden_files.push(index);
        } else if let Some(refs) = sheets.iter().find(|s| s.sheet == file.path) {
            grouping.members.push((index, refs.existing.clone()));
        }
    }
    for (index, path) in failures.iter().enumerate() {
        if is_hidden(path, true) {
            grouping.hidden_failures.push(index);
        }
    }
    grouping
}

fn sheet_refs(sheet: &Path) -> SheetRefs {
    let mut refs = SheetRefs {
        sheet: sheet.to_path_buf(),
        existing: Vec::new(),
        broken_stems: Vec::new(),
    };
    if is_m3u_ext(extension(sheet)) {
        for disc in playlist_entries(sheet).unwrap_or_default() {
            if is_file(&disc) {
                if is_sheet_ext(extension(&disc)) {
                    refs.existing.extend(sheet_refs(&disc).existing);
                }
                refs.existing.push(canonical(&disc));
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

/// The referenced files that exist, canonical.
fn existing(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths
        .iter()
        .filter(|path| is_file(path))
        .map(|path| canonical(path))
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
