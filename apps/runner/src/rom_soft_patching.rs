//! Soft-patching at game start: the `.bps` / `.ups` / `.ips` patches with the ROM's
//! file name in the ROM's folder (numbered ones too: `.ips1`, `.ips2`, …) are applied
//! in memory, in chain order (the ROM file is never modified). Saves stay keyed by
//! the ROM name, so the patched game shares SRAM and states with the unpatched one.
//! A missing, broken or mismatched patch anywhere in the chain is logged and the game
//! starts unpatched (never half-patched).
//!
//! [`content_bytes_for_core`] decides which cartridges are preloaded: patched ones,
//! and ones whose path is too long for cores that copy it into a 256-byte buffer
//! (libretro-host stages preloaded bytes as a short file for cores that load by path).

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use rom_patch::PatchFormat;

/// Outcome for the session: bytes to hand the core (`None` = core reads the ROM
/// file as usual), a short label for the window title and the patch chain's CRC.
#[derive(Debug, Default)]
pub struct SoftPatchedRom {
    pub data: Option<Vec<u8>>,
    pub title_label: Option<String>,
    /// CRC32 of the patch chain found ([`rom_patch::patch_chain_crc32`]), **applied
    /// or not** (state sidecars record it so the launcher can tell the patches
    /// changed); `None` = no patch, or one could not be read.
    pub patch_crc32: Option<u32>,
}

/// Looks for patches next to `rom` and applies them in order.
pub fn load_soft_patched_rom(rom: &Path) -> SoftPatchedRom {
    let paths = rom_patch::find_patches_for_rom(rom);
    if paths.is_empty() {
        return SoftPatchedRom::default();
    }
    let patches = read_all(&paths);
    let patch_crc32 = patches
        .as_ref()
        .ok()
        .and_then(|patches| rom_patch::patch_chain_crc32(patches));
    let result = patches.and_then(|patches| {
        let bytes = std::fs::read(rom).map_err(|err| format!("{}: {err}", rom.display()))?;
        rom_patch::apply_patch_chain(&bytes, &patches)
            .map_err(|(index, err)| format!("{}: {err}", paths[index].display()))
    });
    match result {
        Ok((formats, patched)) => {
            tracing::info!(patches = ?paths, "ROM soft-patched");
            SoftPatchedRom {
                data: Some(patched),
                title_label: Some(chain_label(&formats)),
                patch_crc32,
            }
        }
        Err(err) => {
            tracing::warn!(%err, "patch chain not applied; starting unpatched");
            SoftPatchedRom {
                data: None,
                title_label: Some("patch failed".to_string()),
                patch_crc32,
            }
        }
    }
}

/// Content paths of this many UTF-8 bytes or more are preloaded (Genesis Plus GX
/// cannot open them: 256-byte path buffers, M9 phase-00 spike).
const LONG_PATH_BYTES: usize = 256;

/// Bytes to hand the core for a cartridge: the patched ROM, else the whole file
/// when its path is too long for path-loading cores, else `None` (the core reads
/// `rom` as usual). A read error is logged and leaves the path load to fail.
pub fn content_bytes_for_core<'a>(
    rom: &Path,
    patched: &'a SoftPatchedRom,
) -> Option<Cow<'a, [u8]>> {
    if let Some(bytes) = &patched.data {
        return Some(Cow::Borrowed(bytes));
    }
    if !is_long_path(rom) {
        return None;
    }
    match std::fs::read(rom) {
        Ok(bytes) => Some(Cow::Owned(bytes)),
        Err(err) => {
            tracing::warn!(path = %rom.display(), %err, "cannot preload long-path ROM");
            None
        }
    }
}

fn is_long_path(path: &Path) -> bool {
    path.to_string_lossy().len() >= LONG_PATH_BYTES
}

/// Every patch file's bytes, or the first read error.
fn read_all(paths: &[PathBuf]) -> Result<Vec<Vec<u8>>, String> {
    paths
        .iter()
        .map(|path| std::fs::read(path).map_err(|err| format!("{}: {err}", path.display())))
        .collect()
}

/// Window-title label: "IPS patch" for one patch, "3 patches" for a chain.
fn chain_label(formats: &[PatchFormat]) -> String {
    match formats {
        [format] => format!("{} patch", format.label()),
        _ => format!("{} patches", formats.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_paths_and_patched_roms_are_preloaded() {
        let path = |len: usize| PathBuf::from(format!("{}.md", "x".repeat(len - 3)));
        assert!(!is_long_path(&path(255)));
        assert!(is_long_path(&path(256)));
        assert!(is_long_path(&path(300)));
        let short = Path::new("no/such/Game.md");
        assert!(content_bytes_for_core(short, &SoftPatchedRom::default()).is_none());
        let patched = SoftPatchedRom {
            data: Some(vec![1, 2, 3]),
            ..SoftPatchedRom::default()
        };
        assert_eq!(
            content_bytes_for_core(short, &patched).as_deref(),
            Some(&[1u8, 2, 3][..])
        );
        // Long path that cannot be read: nothing preloaded (the core reports the load).
        assert!(content_bytes_for_core(&path(300), &SoftPatchedRom::default()).is_none());
    }

    #[test]
    fn label_names_one_format_or_counts_a_chain() {
        assert_eq!(chain_label(&[PatchFormat::Ips]), "IPS patch");
        assert_eq!(
            chain_label(&[PatchFormat::Bps, PatchFormat::Ips, PatchFormat::Ips]),
            "3 patches"
        );
    }
}
