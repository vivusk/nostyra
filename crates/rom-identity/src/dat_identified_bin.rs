//! DAT-gated files (Atari 2600 / 7800 `.bin`): a loose file no other system took is
//! listed only when its hashes are in a candidate system's DAT. The caller reads the
//! file (desktop scan, web scan); this decides, without file I/O. Candidates come
//! from `system_catalog::dat_identified_candidates`.

use system_catalog::SystemSpec;

use crate::dat_index::DatIndex;
use crate::rom_file_hasher::{hash_headerless, hash_rom_bytes};

/// The first of `candidates` (in their order) whose DAT `dat_for` knows `bytes`, by
/// the system's primary hashes or its headerless fallback.
pub fn dat_identified_system<'a>(
    bytes: &[u8],
    candidates: &[&'static SystemSpec],
    dat_for: impl Fn(&SystemSpec) -> Option<&'a DatIndex>,
) -> Option<&'static SystemSpec> {
    candidates.iter().copied().find(|spec| {
        dat_for(spec).is_some_and(|dat| {
            dat.lookup(&hash_rom_bytes(bytes, spec)).is_some()
                || hash_headerless(bytes, spec).is_some_and(|bare| dat.lookup(&bare).is_some())
        })
    })
}

/// The problem text for a DAT-gated file no candidate DAT knows: the classifier's
/// `reason` plus which systems were also tried and the extension their homebrew
/// needs ("…; also not a known Atari 2600 / Atari 7800 dump (homebrew: use .a26 /
/// .a78)"). `reason` alone when there are no candidates.
pub fn dat_pending_hint(reason: &str, candidates: &[&'static SystemSpec]) -> String {
    if candidates.is_empty() {
        return reason.to_string();
    }
    let join = |part: &dyn Fn(&SystemSpec) -> String| {
        candidates
            .iter()
            .map(|spec| part(spec))
            .collect::<Vec<_>>()
            .join(" / ")
    };
    let names = join(&|spec| spec.display_name.to_string());
    let exts = join(&|spec| format!(".{}", spec.extensions[0]));
    format!("{reason}; also not a known {names} dump (homebrew: use {exts})")
}

#[cfg(test)]
#[path = "dat_identified_bin_tests.rs"]
mod tests;
