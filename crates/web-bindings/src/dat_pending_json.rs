//! DAT-gated loose files on the web (Atari 2600 / 7800 `.bin`, M17): which systems a
//! file the classifier rejected may still be, whether one system's DAT knows it and
//! the problem hint for a miss — the desktop scanner's rules (`game-library`
//! `dat_pending_resolution`), on paths of the current content source.

use std::io::Read;
use std::path::Path;

use rom_identity::{dat_identified_system, dat_pending_hint, DatIndex};
use system_catalog::{dat_identified_candidates, system, ALL_SYSTEMS};

/// Ids of the systems a DAT may still give the file at `path` (extension + size), in
/// registry order; empty when none (or the file cannot be read).
pub fn candidates(path: &Path) -> Vec<String> {
    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return Vec::new();
    };
    let Ok(len) = content_file_source::len(path) else {
        return Vec::new();
    };
    dat_identified_candidates(&ALL_SYSTEMS, ext, len)
        .iter()
        .map(|spec| spec.id.to_string())
        .collect()
}

/// True when `dat` (the DAT of `system_id`) lists the file at `path`, by its primary
/// hashes or the headerless fallback. The file is read whole, capped at the system's
/// size limit.
pub fn dat_hit(path: &Path, system_id: &str, dat: &DatIndex) -> Result<bool, String> {
    let spec = system(system_id).ok_or_else(|| format!("unknown system {system_id}"))?;
    let mut bytes = Vec::new();
    content_file_source::open(path)
        .and_then(|file| file.take(spec.max_bytes + 1).read_to_end(&mut bytes))
        .map_err(|err| err.to_string())?;
    Ok(dat_identified_system(&bytes, &[spec], |_| Some(dat)).is_some())
}

/// The problem text for a pending file no candidate DAT knows (desktop wording).
pub fn hint(reason: &str, candidate_ids: &[String]) -> String {
    let specs: Vec<_> = candidate_ids.iter().filter_map(|id| system(id)).collect();
    dat_pending_hint(reason, &specs)
}

#[cfg(test)]
#[path = "dat_pending_json_tests.rs"]
mod tests;
