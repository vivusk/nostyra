//! Decides which system a file belongs to from its extension, size and first bytes.
//! Folder names are never read: users organise ROM folders any way they like
//! (`ROM/Favorites/mario.nes`, an `.sfc` inside `ROM/NES/`), so only content counts.

use std::fs::File;
use std::io::Read;
use std::path::Path;

pub use crate::classify_error::ClassifyError;
use crate::disc_probe::DiscProbe;
use crate::foreign_executable_probe::foreign_elf;
use crate::rom_header_rules::{ContentProbe, PROBE_BYTES};
use crate::system_specs::{SystemSpec, ALL_SYSTEMS};

/// Systems using `ext` (no dot, any case), in registry order.
pub fn candidates_for_extension(ext: &str) -> Vec<&'static SystemSpec> {
    ALL_SYSTEMS
        .into_iter()
        .filter(|spec| spec.has_extension(ext))
        .collect()
}

/// Classifies the file at `path` (extension `ext`, whole-file length `file_len`)
/// among every registry system. See [`classify_among`].
pub fn classify(
    ext: &str,
    file_len: u64,
    path: &Path,
    disc: &dyn DiscProbe,
) -> Result<&'static SystemSpec, ClassifyError> {
    classify_among(&ALL_SYSTEMS, ext, file_len, path, disc)
}

/// [`classify`] restricted to `systems` (kept in the given order). First candidate
/// passing size + probe wins; a probe failure beats an out-of-size result in the
/// reported error. The file is opened only when a candidate of the right size
/// checks content: the first [`PROBE_BYTES`] once for magic probes, `disc` for disc
/// signatures.
pub fn classify_among(
    systems: &[&'static SystemSpec],
    ext: &str,
    file_len: u64,
    path: &Path,
    disc: &dyn DiscProbe,
) -> Result<&'static SystemSpec, ClassifyError> {
    classify_with_head(systems, ext, file_len, None, path, disc)
}

/// [`classify_among`] with the file head already in memory (`Some`: never read
/// from `path`; tests and callers that hold the bytes).
pub fn classify_with_head(
    systems: &[&'static SystemSpec],
    ext: &str,
    file_len: u64,
    head: Option<&[u8]>,
    path: &Path,
    disc: &dyn DiscProbe,
) -> Result<&'static SystemSpec, ClassifyError> {
    let mut read_head: Option<Vec<u8>> = None;
    let mut error = ClassifyError::NoCandidate;
    for spec in systems
        .iter()
        .copied()
        .filter(|spec| spec.has_extension(ext))
    {
        if !spec.accepts_size(file_len) {
            if error == ClassifyError::NoCandidate {
                error = ClassifyError::OutOfSize;
            }
            continue;
        }
        let probe = spec.probe;
        let bytes = if probe.needs_bytes(ext) {
            Some(head_bytes(head, &mut read_head, path)?)
        } else {
            None
        };
        let result = match (probe, bytes) {
            // A GameCube / Wii `.elf` built for another CPU is no ROM at all.
            (ContentProbe::Disc(signature), bytes) => match bytes.and_then(foreign_elf) {
                Some(reason) => Err(ClassifyError::NotRom(reason)),
                None => disc
                    .check(signature, path)
                    .map_err(ClassifyError::DiscRejected),
            },
            (_, Some(bytes)) => probe.check(ext, bytes).map_err(|reason| {
                if probe.is_quiet(ext) {
                    ClassifyError::NotRom(reason)
                } else {
                    ClassifyError::ProbeFailed(reason)
                }
            }),
            (_, None) => Ok(()),
        };
        match result {
            Ok(()) => return Ok(spec),
            Err(reason) if error.outranks(&reason) => {}
            Err(reason) => error = reason,
        }
    }
    Err(error)
}

/// The first [`PROBE_BYTES`] of the file: `head` when given, else read once from
/// `path` into `read_head`.
fn head_bytes<'a>(
    head: Option<&'a [u8]>,
    read_head: &'a mut Option<Vec<u8>>,
    path: &Path,
) -> Result<&'a [u8], ClassifyError> {
    if let Some(bytes) = head {
        return Ok(bytes);
    }
    if read_head.is_none() {
        let bytes = read_probe_bytes(path).map_err(|err| ClassifyError::Io(err.to_string()))?;
        *read_head = Some(bytes);
    }
    Ok(read_head.as_deref().unwrap_or_default())
}

/// Reads the first [`PROBE_BYTES`] of `path` (fewer for a shorter file).
pub fn read_probe_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut first = Vec::with_capacity(PROBE_BYTES);
    File::open(path)?
        .take(PROBE_BYTES as u64)
        .read_to_end(&mut first)?;
    Ok(first)
}

/// Classifies a file on disk by extension, length and content. Used by the runner
/// when no `--system` is given.
pub fn classify_path(
    path: &Path,
    disc: &dyn DiscProbe,
) -> Result<&'static SystemSpec, ClassifyError> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .ok_or(ClassifyError::NoCandidate)?;
    if candidates_for_extension(ext).is_empty() {
        return Err(ClassifyError::NoCandidate);
    }
    let file_len = std::fs::metadata(path)
        .map_err(|err| ClassifyError::Io(err.to_string()))?
        .len();
    classify(ext, file_len, path, disc)
}

#[cfg(test)]
#[path = "rom_classifier_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "rom_classifier_sega_tests.rs"]
mod sega_tests;

#[cfg(test)]
#[path = "rom_classifier_n64_tests.rs"]
mod n64_tests;

#[cfg(test)]
#[path = "rom_classifier_nds_tests.rs"]
mod nds_tests;
