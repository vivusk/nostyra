//! Finds the patches for a ROM: same folder, same file name, patch extension,
//! optionally numbered to chain several patches of one format.

use std::path::{Path, PathBuf};

/// Formats in chain order: every BPS first, then UPS, then IPS.
pub const PATCH_EXTENSIONS: [&str; 3] = ["bps", "ups", "ips"];

/// The patch chain for `Game.sfc`, in the order it is applied:
/// `Game.bps`, `Game.bps1`, `Game.bps2`, …, then the same for `ups`, then `ips`
/// (as RetroArch numbers them). Per format the unnumbered file is optional and
/// numbering starts at 1 and stops at the first gap (`ips1`, `ips3` → only `ips1`).
/// Upper-case extensions (`Game.IPS2`) count too.
pub fn find_patches_for_rom(rom: &Path) -> Vec<PathBuf> {
    let mut chain = Vec::new();
    for ext in PATCH_EXTENSIONS {
        chain.extend(existing(rom, ext));
        for number in 1.. {
            match existing(rom, &format!("{ext}{number}")) {
                Some(path) => chain.push(path),
                None => break,
            }
        }
    }
    chain
}

/// `rom` with extension `ext` (lower case first, then upper case), if it is a file.
/// One path per extension: on case-insensitive file systems both spellings match.
fn existing(rom: &Path, ext: &str) -> Option<PathBuf> {
    [ext.to_string(), ext.to_ascii_uppercase()]
        .into_iter()
        .map(|ext| rom.with_extension(ext))
        .find(|candidate| candidate.is_file())
}
