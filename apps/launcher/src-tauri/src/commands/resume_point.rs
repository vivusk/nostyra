//! Reads a game's resume point (`auto.*`, written by the runner on clean exits)
//! and decides which warnings force a conscious Continue / Start fresh choice.
//! File-only (no Tauri), so every case is tested against scratch folders.

use std::path::Path;

use ipc_protocol::{
    state_file, StateSidecar, StateSlotId, SIDECAR_FILE_EXT, SRAM_FILE_NAME, STATE_FILE_EXT,
    THUMBNAIL_FILE_EXT,
};
use serde::Serialize;

use crate::commands::state_load_rule::StateLoadRule;

/// Why resuming may not be what the player wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResumeWarning {
    /// `save.srm` changed since the resume point: in-game progress is newer.
    SramNewer,
    /// The soft-patch file next to the ROM changed (added, removed or edited).
    PatchChanged,
    /// The installed core's version differs from the one that saved.
    CoreVersionChanged,
    /// The runner would refuse this state (strict-version core saved by another
    /// version, or another disc / revision): Start fresh only.
    StateIncompatible,
}

/// A usable resume point: `auto.state` plus a readable sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePoint {
    pub created_at: i64,
    /// Absolute `auto.png` path, if written.
    pub thumbnail_path: Option<String>,
    pub warnings: Vec<ResumeWarning>,
    /// Why it cannot be loaded (with [`ResumeWarning::StateIncompatible`]).
    pub incompatible_reason: Option<String>,
}

/// The resume point in `saves_dir`, or `None` without `auto.state` or a valid
/// sidecar (a sidecar from another core is left to the runner's own check).
/// `patch_crc32` = CRC of the patch file found for the ROM now (`None` = none);
/// `rule` = the installed core version (`None` = unknown) + strict / content rules.
pub fn read_resume_point(
    saves_dir: &Path,
    patch_crc32: Option<u32>,
    rule: &StateLoadRule,
) -> Option<ResumePoint> {
    let auto = |ext| state_file(saves_dir, StateSlotId::Auto, ext);
    if !auto(STATE_FILE_EXT).is_file() {
        return None;
    }
    let raw = std::fs::read(auto(SIDECAR_FILE_EXT)).ok()?;
    let sidecar: StateSidecar = serde_json::from_slice(&raw).ok()?;
    let sram_crc32 = std::fs::read(saves_dir.join(SRAM_FILE_NAME))
        .ok()
        .map(|bytes| crc32fast::hash(&bytes));
    let thumbnail = auto(THUMBNAIL_FILE_EXT);
    Some(ResumePoint {
        created_at: i64::try_from(sidecar.created_at).unwrap_or(i64::MAX),
        thumbnail_path: thumbnail
            .is_file()
            .then(|| thumbnail.to_string_lossy().into_owned()),
        warnings: resume_warnings(&sidecar, sram_crc32, patch_crc32, rule),
        incompatible_reason: rule.incompatible_reason(&sidecar),
    })
}

/// Warnings for `sidecar` given today's facts. No `.srm` (the game never wrote
/// SRAM) or no recorded SRAM CRC → no `sram-newer`; an unknown core version on
/// either side → no `core-version-changed`; a strict-version core reports a version
/// change as `state-incompatible` instead (as does another disc / revision).
pub fn resume_warnings(
    sidecar: &StateSidecar,
    sram_crc32: Option<u32>,
    patch_crc32: Option<u32>,
    rule: &StateLoadRule,
) -> Vec<ResumeWarning> {
    let mut warnings = Vec::new();
    if let (Some(saved), Some(now)) = (sidecar.sram_crc32, sram_crc32) {
        if saved != now {
            warnings.push(ResumeWarning::SramNewer);
        }
    }
    if sidecar.rom_patch_crc32 != patch_crc32 {
        warnings.push(ResumeWarning::PatchChanged);
    }
    let saved_version = Some(sidecar.core_version.as_str()).filter(|v| !v.is_empty());
    if let (Some(saved), Some(now)) = (saved_version, rule.installed_version.as_deref()) {
        if saved != now && !rule.strict_version {
            warnings.push(ResumeWarning::CoreVersionChanged);
        }
    }
    if rule.incompatible_reason(sidecar).is_some() {
        warnings.push(ResumeWarning::StateIncompatible);
    }
    warnings
}

/// CRC32 of the soft-patch chain the runner would apply to `rom` (`None` when there
/// is none or a patch cannot be read; the runner records the same).
pub fn current_patch_crc32(rom: &Path) -> Option<u32> {
    let patches = rom_patch::find_patches_for_rom(rom)
        .iter()
        .map(std::fs::read)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    rom_patch::patch_chain_crc32(&patches)
}

#[cfg(test)]
#[path = "resume_point_tests.rs"]
mod tests;
