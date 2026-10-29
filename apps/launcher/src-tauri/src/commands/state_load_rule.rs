//! The launcher's copy of the runner's state refusal rule (`state_file_guard`),
//! so it never offers a state the runner would refuse: Continue is hidden and the slot
//! is marked when the game's core has `strict_state_version` and the state was saved
//! by another installed version, or when the state was made from another disc /
//! revision sharing the save folder (content tag, GameCube / Wii). Both sides use
//! `ipc_protocol::state_incompatibility` and `game_library::state_content_tag`.

use ipc_protocol::{state_incompatibility, StateSidecar};

use crate::app_state::AppState;
use crate::command_error::CommandResult;
use crate::commands::game_data_target::GameDataTarget;

/// What a state is checked against for one game (lenient + untagged by default).
#[derive(Debug, Clone, Default)]
pub struct StateLoadRule {
    /// `CoreSpec::strict_state_version` of the game's registry core.
    pub strict_version: bool,
    /// Registry display name of that core ("Dolphin"), for the reason text.
    pub core_display_name: &'static str,
    /// The installed core's version as recorded by the launcher (`None` = unknown).
    pub installed_version: Option<String>,
    /// The game's content tag (`None` = untagged system).
    pub content_tag: Option<String>,
}

impl StateLoadRule {
    /// The rule for `target`: its registry core, installed version and content tag.
    pub fn for_game(state: &AppState, target: &GameDataTarget) -> CommandResult<Self> {
        let Some(core) = system_catalog::core_for_system(&target.system) else {
            return Ok(Self::default());
        };
        let installed = storage_db::get_installed_core(&state.db(), core.id)?;
        let content_tag = system_catalog::system(&target.system)
            .and_then(|system| game_library::state_content_tag(system, &target.rom_path));
        Ok(Self {
            strict_version: core.strict_state_version,
            core_display_name: core.display_name,
            installed_version: installed.and_then(|record| record.version),
            content_tag,
        })
    }

    /// Why the runner would refuse `sidecar` ("Made by another Dolphin version"),
    /// or `None` when it may load.
    pub fn incompatible_reason(&self, sidecar: &StateSidecar) -> Option<String> {
        let why = state_incompatibility(
            sidecar,
            self.strict_version,
            self.installed_version.as_deref(),
            self.content_tag.as_deref(),
        )?;
        let reason = why.reason(self.core_display_name);
        let mut chars = reason.chars();
        Some(chars.next().map_or(String::new(), |first| {
            first.to_uppercase().chain(chars).collect()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sidecar(version: &str, tag: Option<&str>) -> StateSidecar {
        StateSidecar {
            core_name: "dolphin-emu".into(),
            core_version: version.into(),
            content_tag: tag.map(str::to_owned),
            ..StateSidecar::default()
        }
    }

    fn rule(strict: bool, version: Option<&str>, tag: Option<&str>) -> StateLoadRule {
        StateLoadRule {
            strict_version: strict,
            core_display_name: "Dolphin",
            installed_version: version.map(str::to_owned),
            content_tag: tag.map(str::to_owned),
        }
    }

    #[test]
    fn strict_version_mismatch_is_incompatible_lenient_or_unknown_is_not() {
        let saved = sidecar("2609", None);
        assert_eq!(
            rule(true, Some("2610"), None)
                .incompatible_reason(&saved)
                .as_deref(),
            Some("Made by another Dolphin version")
        );
        assert_eq!(
            rule(false, Some("2610"), None).incompatible_reason(&saved),
            None
        );
        assert_eq!(rule(true, None, None).incompatible_reason(&saved), None);
        assert_eq!(
            rule(true, Some("2609"), None).incompatible_reason(&saved),
            None
        );
    }

    #[test]
    fn other_disc_state_is_incompatible() {
        let disc2 = sidecar("1", Some("GALE01:1:0"));
        let disc1 = rule(true, Some("1"), Some("GALE01:0:0"));
        assert_eq!(
            disc1.incompatible_reason(&disc2).as_deref(),
            Some("Made from another disc or revision")
        );
        assert_eq!(disc1.incompatible_reason(&sidecar("1", None)), None);
        assert_eq!(StateLoadRule::default().incompatible_reason(&disc2), None);
    }
}
