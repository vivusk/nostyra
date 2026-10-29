//! The system registry as the UI sees it: every system (labels, extensions, pad
//! actions, core) and the systems the library has games of (Library filter pills).

use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;
use crate::command_error::CommandResult;

/// One registry system.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemView {
    pub id: &'static str,
    pub display_name: &'static str,
    pub short_label: &'static str,
    pub extensions: &'static [&'static str],
    /// RetroPad actions its pad has, in UI order (Controls editor subset).
    pub pad_actions: &'static [&'static str],
    /// The system's own button names (Mega Drive `y` → "A"); empty = RetroPad names.
    pub pad_labels: Vec<PadLabelView>,
    pub core_id: &'static str,
}

/// One system button name for a RetroPad action.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PadLabelView {
    pub action: &'static str,
    pub label: &'static str,
}

/// A system with games in the library.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySystem {
    pub id: &'static str,
    pub display_name: &'static str,
    pub short_label: &'static str,
    pub count: i64,
}

/// Every registry system, in registry order (SNES first).
#[tauri::command]
pub fn get_systems() -> Vec<SystemView> {
    system_catalog::ALL_SYSTEMS
        .iter()
        .map(|spec| SystemView {
            id: spec.id,
            display_name: spec.display_name,
            short_label: spec.short_label,
            extensions: spec.extensions,
            pad_actions: spec.pad_actions,
            pad_labels: spec
                .pad_labels
                .iter()
                .map(|&(action, label)| PadLabelView { action, label })
                .collect(),
            core_id: spec.default_core_id,
        })
        .collect()
}

/// Registry systems with visible games, with counts, in display order (`DISPLAY_ORDER`).
#[tauri::command]
pub fn get_library_systems(state: State<'_, AppState>) -> CommandResult<Vec<LibrarySystem>> {
    let counts = storage_db::library_platform_counts(&state.db())?;
    Ok(library_systems(&counts))
}

fn library_systems(counts: &[(String, i64)]) -> Vec<LibrarySystem> {
    system_catalog::DISPLAY_ORDER
        .iter()
        .filter_map(|spec| {
            let (_, count) = counts.iter().find(|(id, _)| id == spec.id)?;
            Some(LibrarySystem {
                id: spec.id,
                display_name: spec.display_name,
                short_label: spec.short_label,
                count: *count,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_actions_are_retropad_actions() {
        // The registry is std-only, so its parity with the binding names lives here.
        // Every RetroPad action is some system's button (L2/R2: PlayStation).
        for action in config_layers::PAD_ACTIONS {
            assert!(
                system_catalog::ALL_SYSTEMS
                    .iter()
                    .any(|s| s.pad_actions.contains(&action)),
                "{action} unused"
            );
        }
        for system in system_catalog::ALL_SYSTEMS {
            for action in system.pad_actions {
                assert!(
                    config_layers::PAD_ACTIONS.contains(action),
                    "{}: {action}",
                    system.id
                );
            }
        }
    }

    #[test]
    fn library_systems_follow_display_order_and_skip_unknown_ids() {
        let counts = [
            ("dreamcast".to_string(), 1),
            ("gba".to_string(), 3),
            ("no-such-system".to_string(), 4),
            ("psx".to_string(), 2),
            ("snes".to_string(), 5),
            ("nes".to_string(), 2),
            ("gc".to_string(), 1),
        ];
        let systems = library_systems(&counts);
        let shape: Vec<(&str, i64)> = systems.iter().map(|s| (s.id, s.count)).collect();
        assert_eq!(
            shape,
            [
                ("nes", 2),
                ("snes", 5),
                ("gc", 1),
                ("gba", 3),
                ("psx", 2),
                ("dreamcast", 1)
            ]
        );
    }

    #[test]
    fn systems_carry_their_button_names() {
        let systems = get_systems();
        let view = |id: &str| systems.iter().find(|s| s.id == id).unwrap();
        let genesis = view(system_catalog::GENESIS.id);
        assert_eq!(genesis.pad_labels.len(), 7);
        assert_eq!(
            genesis.pad_labels[0],
            PadLabelView {
                action: "y",
                label: "A"
            }
        );
        assert!(view(system_catalog::SNES.id).pad_labels.is_empty());
        let json = serde_json::to_value(genesis).unwrap();
        assert_eq!(json["padLabels"][6]["label"], "Mode");
    }

    #[test]
    fn gamecube_and_wii_rows_carry_the_registry_labels() {
        // Controls rows = the runner's pad actions, labels as the registry names them.
        let systems = get_systems();
        for spec in [system_catalog::GC, system_catalog::WII] {
            let view = systems.iter().find(|s| s.id == spec.id).unwrap();
            assert_eq!(view.pad_actions, spec.pad_actions, "{}", spec.id);
            let labels: Vec<(&str, &str)> = view
                .pad_labels
                .iter()
                .map(|l| (l.action, l.label))
                .collect();
            assert_eq!(labels, spec.pad_labels, "{}", spec.id);
        }
        let wii = systems.iter().find(|s| s.id == system_catalog::WII.id);
        let wii = serde_json::to_value(wii.unwrap()).unwrap();
        let tilt = &wii["padLabels"].as_array().unwrap().last().unwrap()["label"];
        assert_eq!(tilt, "Tilt Remote Right");
    }
}
