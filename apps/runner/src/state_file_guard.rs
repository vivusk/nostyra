//! Host-side checks of a save state before its bytes reach the core, from its
//! sidecar (`StateSidecar`): the core name (states are core-specific), the core
//! version (warning only, or refused for `strict_state_version` cores), the content
//! tag (another disc / revision sharing the save folder, see
//! `game_library::state_content_tag`), then the file length and CRC32 recorded at
//! save time (a truncated, corrupted or half-written state). Dolphin accepts any blob
//! in `retro_unserialize` and goes on with a broken machine (M14 spike), so these
//! refusals are the only protection. Sidecars written before M14 carry no length,
//! CRC or tag: those checks are skipped for them.

use anyhow::bail;
use ipc_protocol::{state_incompatibility, StateSidecar};
use libretro_host::CoreInfo;
use system_catalog::SystemSpec;

/// Refusal text for a state whose length or CRC differs from its sidecar.
pub const STATE_DAMAGED: &str = "state file is damaged";

/// The session facts a state is checked against (lenient, untagged by default).
#[derive(Debug, Clone, Default)]
pub struct StateFileGuard {
    /// `CoreSpec::strict_state_version` of the system's core.
    strict_version: bool,
    /// Registry display name of the core ("Dolphin"), for the refusal text.
    core_display_name: &'static str,
    /// The launched game's content tag (`None` = no tag check).
    content_tag: Option<String>,
}

impl StateFileGuard {
    /// Guard for `system`'s registry core and the launched game's `content_tag`.
    pub fn new(system: &SystemSpec, content_tag: Option<String>) -> Self {
        let core = system_catalog::core_for_system(system.id);
        Self {
            strict_version: core.is_some_and(|core| core.strict_state_version),
            core_display_name: core.map_or("core", |core| core.display_name),
            content_tag,
        }
    }

    /// Records what a later load checks: the state's length, CRC32 and content tag.
    pub fn stamp(&self, sidecar: &mut StateSidecar, state: &[u8]) {
        sidecar.state_len = Some(state.len() as u64);
        sidecar.state_crc32 = Some(crc32fast::hash(state));
        sidecar.content_tag.clone_from(&self.content_tag);
    }

    /// Before the state file is read: core name, version, content tag.
    pub fn check_sidecar(&self, sidecar: &StateSidecar, core: &CoreInfo) -> anyhow::Result<()> {
        if sidecar.core_name != core.name {
            bail!(
                "state was made by core '{}', current core is '{}'",
                sidecar.core_name,
                core.name
            );
        }
        let tag = self.content_tag.as_deref();
        let version = Some(core.version.as_str());
        if let Some(why) = state_incompatibility(sidecar, self.strict_version, version, tag) {
            bail!("state was {}", why.reason(self.core_display_name));
        }
        if sidecar.core_version != core.version {
            tracing::warn!(saved = %sidecar.core_version, current = %core.version,
                "state from a different core version; trying to load");
        }
        Ok(())
    }

    /// Before the state file is read: its length against the recorded one.
    pub fn check_len(sidecar: &StateSidecar, len: u64) -> anyhow::Result<()> {
        if sidecar.state_len.is_some_and(|saved| saved != len) {
            bail!(
                "{STATE_DAMAGED} (length {len}, saved {:?})",
                sidecar.state_len
            );
        }
        Ok(())
    }

    /// After reading, before the core: the bytes against the recorded CRC32.
    pub fn check_crc(sidecar: &StateSidecar, state: &[u8]) -> anyhow::Result<()> {
        if sidecar
            .state_crc32
            .is_some_and(|saved| saved != crc32fast::hash(state))
        {
            bail!("{STATE_DAMAGED} (CRC mismatch)");
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "state_file_guard_tests.rs"]
mod tests;
