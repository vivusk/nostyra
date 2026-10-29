//! Launcher hardcore gating (flag forced on by passing `hardcore_ui`, as the debug
//! env `NOSTYRA_RA_HARDCORE_UI=1` does): the launch rule, slot marking and the
//! Play arguments.

use super::*;

fn config(enabled: bool, hardcore: bool) -> AchievementsConfig {
    AchievementsConfig {
        enabled,
        hardcore,
        ..AchievementsConfig::default()
    }
}

fn slot(id: StateSlotId, reason: Option<&str>) -> SaveStateSlot {
    SaveStateSlot {
        slot: id,
        created_at: 1,
        core_name: None,
        core_version: None,
        thumbnail_path: None,
        size_bytes: 1,
        incompatible_reason: reason.map(str::to_owned),
    }
}

#[test]
fn hardcore_needs_flag_settings_login_and_an_ra_system() {
    let nes = system_catalog::NES.id;
    let gamecube = system_catalog::GC.id;
    assert!(hardcore_effective(&config(true, true), nes, true, true));
    assert!(
        !hardcore_effective(&config(true, true), nes, true, false),
        "hidden flag → softcore"
    );
    assert!(!hardcore_effective(&config(true, false), nes, true, true));
    assert!(!hardcore_effective(&config(false, true), nes, true, true));
    assert!(!hardcore_effective(&config(true, true), nes, false, true));
    assert!(
        !hardcore_effective(&config(true, true), gamecube, true, true),
        "no RA console"
    );
    assert!(!hardcore_effective(
        &config(true, true),
        "unknown",
        true,
        true
    ));
}

#[test]
fn hardcore_marks_every_slot_unloadable_and_keeps_existing_reasons() {
    let mut slots = vec![
        slot(StateSlotId::Auto, None),
        slot(StateSlotId::Numbered(3), None),
        slot(
            StateSlotId::Numbered(4),
            Some("Made by another Dolphin version"),
        ),
    ];
    block_loads_in_hardcore(&mut slots);
    let reasons: Vec<_> = slots
        .iter()
        .map(|s| s.incompatible_reason.as_deref())
        .collect();
    assert_eq!(
        reasons,
        [
            Some(RESUME_DISABLED),
            Some(SLOT_LOAD_DISABLED),
            Some("Made by another Dolphin version")
        ]
    );
}

#[test]
fn play_never_loads_a_state_in_hardcore() {
    let auto = Some(StateSlotId::Auto);
    assert_eq!(launch_load_slot(auto, true), None);
    assert_eq!(launch_load_slot(Some(StateSlotId::Numbered(2)), true), None);
    assert_eq!(launch_load_slot(auto, false), auto);
    assert_eq!(launch_load_slot(None, false), None);
}

#[test]
fn capabilities_follow_the_hidden_flag() {
    let expected = achievements::hardcore_ui_enabled();
    assert_eq!(achievements_capabilities().hardcore_ui, expected);
    let json = serde_json::to_string(&achievements_capabilities()).unwrap_or_default();
    assert_eq!(json, format!("{{\"hardcoreUi\":{expected}}}"));
}

#[test]
fn game_rule_reads_the_global_layer_and_the_login_file() {
    let data = tempfile::tempdir().unwrap();
    let root = data.path();
    let target = GameDataTarget {
        system: system_catalog::NES.id.to_owned(),
        rom_path: root.join("game.nes"),
        content_sha1: "0".repeat(40),
        missing: false,
    };
    let global = config_layers::global_config_path(root);
    std::fs::create_dir_all(global.parent().unwrap()).unwrap();
    std::fs::write(
        &global,
        "[achievements]
hardcore = true
",
    )
    .unwrap();
    assert!(!hardcore_for_game_with(root, &target, true), "logged out");
    let login = achievements::Credentials {
        user: "Tester".into(),
        token: "tok".into(),
    };
    achievements::save_credentials(root, &login).unwrap();
    assert!(hardcore_for_game_with(root, &target, true));
    assert!(!hardcore_for_game_with(root, &target, false), "hidden flag");
}
