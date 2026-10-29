//! RetroAchievements mode facts for the launcher (M15 phase 04). The launcher has no
//! live RA client, so it predicts the runner's launch-time hardcore decision from
//! the same inputs: the hidden UI flag (`achievements::hardcore_ui_enabled`), the
//! global `[achievements]` settings, a stored login and the system's RA console.
//! When hardcore would be effective, Continue is hidden, slot Load is disabled (the
//! list stays) and `play_game` never passes `--load-state` (the runner refuses
//! anyway). `achievements_capabilities` tells the UI whether the hardcore toggle
//! exists at all (Settings, phase 06).

use std::path::Path;

use config_layers::AchievementsConfig;
use ipc_protocol::StateSlotId;
use serde::Serialize;

use crate::commands::game_data_target::GameDataTarget;
use crate::commands::save_state_commands::SaveStateSlot;

/// Reason on the resume point's card while hardcore is effective.
pub const RESUME_DISABLED: &str = "Hardcore: resume disabled";
/// Reason on a numbered slot's card while hardcore is effective.
pub const SLOT_LOAD_DISABLED: &str = "Hardcore: loading states disabled";

/// What the achievements UI may offer in this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementsCapabilities {
    /// The hardcore toggle is shown (false until RetroAchievements approves Nostyra).
    pub hardcore_ui: bool,
}

#[tauri::command]
pub fn achievements_capabilities() -> AchievementsCapabilities {
    AchievementsCapabilities {
        hardcore_ui: achievements::hardcore_ui_enabled(),
    }
}

/// The runner's launch rule: hardcore needs the flag, achievements on, hardcore on,
/// a login and a system RetroAchievements supports.
pub fn hardcore_effective(
    config: &AchievementsConfig,
    system_id: &str,
    logged_in: bool,
    hardcore_ui: bool,
) -> bool {
    hardcore_ui
        && config.enabled
        && config.hardcore
        && logged_in
        && system_catalog::system(system_id).is_some_and(|system| system.ra_console_id.is_some())
}

/// Whether the next launch of `target` would run in hardcore (reads the config and
/// the credentials file only when the flag is on).
pub fn hardcore_for_game(data_root: &Path, target: &GameDataTarget) -> bool {
    hardcore_for_game_with(data_root, target, achievements::hardcore_ui_enabled())
}

fn hardcore_for_game_with(data_root: &Path, target: &GameDataTarget, hardcore_ui: bool) -> bool {
    if !hardcore_ui {
        return false;
    }
    let (config, _) = target.resolve_with_warnings(data_root);
    let logged_in = matches!(achievements::load_credentials(data_root), Ok(Some(_)));
    hardcore_effective(&config.achievements, &target.system, logged_in, hardcore_ui)
}

/// Marks every listed state as not loadable in hardcore (an existing refusal reason
/// is kept); Delete stays available.
pub fn block_loads_in_hardcore(slots: &mut [SaveStateSlot]) {
    for slot in slots {
        let reason = match slot.slot {
            StateSlotId::Auto => RESUME_DISABLED,
            StateSlotId::Numbered(_) => SLOT_LOAD_DISABLED,
        };
        slot.incompatible_reason
            .get_or_insert_with(|| reason.to_owned());
    }
}

/// The slot `play_game` may load at start: none in hardcore (logged).
pub fn launch_load_slot(slot: Option<StateSlotId>, hardcore: bool) -> Option<StateSlotId> {
    if hardcore && slot.is_some() {
        tracing::info!("hardcore: game starts without loading a state");
        return None;
    }
    slot
}

#[cfg(test)]
#[path = "achievements_mode_tests.rs"]
mod tests;
