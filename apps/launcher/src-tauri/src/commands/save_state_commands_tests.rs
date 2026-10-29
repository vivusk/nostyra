//! Slot listing / deletion against a scratch save folder.

use super::*;

fn scratch(name: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-slots-{name}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

fn write(dir: &Path, name: &str, bytes: &[u8]) {
    std::fs::write(dir.join(name), bytes).unwrap();
}

fn sidecar_json(created_at: u64) -> Vec<u8> {
    serde_json::to_vec(&StateSidecar {
        core_name: "Snes9x".into(),
        core_version: "1.63".into(),
        created_at,
        ..StateSidecar::default()
    })
    .unwrap()
}

#[test]
fn lists_used_slots_with_sidecar_or_mtime_and_optional_thumbnail() {
    let temp = scratch("list");
    let dir = temp.path();
    // Slot 1: full set. Slot 4: state only. Slot 6: sidecar + png, no state.
    write(dir, "slot1.state", &[0; 100]);
    write(dir, "slot1.json", &sidecar_json(1_700_000_000));
    write(dir, "slot1.png", b"png");
    write(dir, "slot4.state", &[0; 7]);
    write(dir, "slot6.json", &sidecar_json(1));
    write(dir, "slot6.png", b"png");
    // Not slots: SRAM, out-of-range slot, temp file, other names.
    for name in ["game.srm", "slot10.state", "slot2.state.tmp", "slotx.state"] {
        write(dir, name, b"x");
    }

    let slots = list_slots(dir, &StateLoadRule::default());
    assert_eq!(slots.len(), 2, "{slots:?}");
    let full = &slots[0];
    assert_eq!(
        (full.slot, full.created_at, full.size_bytes),
        (StateSlotId::Numbered(1), 1_700_000_000, 100)
    );
    assert_eq!(full.core_name.as_deref(), Some("Snes9x"));
    assert_eq!(full.core_version.as_deref(), Some("1.63"));
    assert_eq!(
        full.thumbnail_path,
        Some(dir.join("slot1.png").to_string_lossy().into_owned())
    );

    let bare = &slots[1];
    assert_eq!((bare.slot, bare.size_bytes), (StateSlotId::Numbered(4), 7));
    assert_eq!(
        (bare.core_name.clone(), bare.thumbnail_path.clone()),
        (None, None)
    );
    let now = crate::app_state::unix_now();
    assert!(
        (now - 60..=now + 60).contains(&bare.created_at),
        "mtime {}",
        bare.created_at
    );
}

#[test]
fn missing_folder_lists_nothing() {
    assert!(list_slots(
        &std::env::temp_dir().join("nostyra-no-such-saves"),
        &StateLoadRule::default()
    )
    .is_empty());
}

#[test]
fn delete_removes_the_three_files_only() {
    let temp = scratch("delete");
    let dir = temp.path();
    for name in [
        "slot3.state",
        "slot3.json",
        "slot3.png",
        "slot2.state",
        "game.srm",
    ] {
        write(dir, name, b"x");
    }
    delete_slot_files(dir, StateSlotId::Numbered(3)).unwrap();
    for gone in ["slot3.state", "slot3.json", "slot3.png"] {
        assert!(!dir.join(gone).exists(), "{gone}");
    }
    assert!(dir.join("slot2.state").exists() && dir.join("game.srm").exists());
    // Already empty (or partial) slots delete without error.
    delete_slot_files(dir, StateSlotId::Numbered(3)).unwrap();
}

#[test]
fn delete_while_the_game_runs_is_busy() {
    assert_eq!(ensure_idle(Some(5), 5).unwrap_err().kind, "busy");
}

#[test]
fn resume_point_lists_first_and_deletes_alone() {
    let temp = scratch("auto");
    let dir = temp.path();
    for name in ["slot0.state", "auto.state", "auto.png", "slot9.state"] {
        write(dir, name, b"x");
    }
    write(dir, "auto.json", &sidecar_json(5));
    let slots: Vec<StateSlotId> = list_slots(dir, &StateLoadRule::default())
        .into_iter()
        .map(|s| s.slot)
        .collect();
    assert_eq!(
        slots,
        [
            StateSlotId::Auto,
            StateSlotId::Numbered(0),
            StateSlotId::Numbered(9)
        ]
    );
    let json = serde_json::to_value(&list_slots(dir, &StateLoadRule::default())[0]).unwrap();
    assert_eq!(json["slot"], "auto");
    assert_eq!(json["createdAt"], 5);
    delete_slot_files(dir, StateSlotId::Auto).unwrap();
    for gone in ["auto.state", "auto.json", "auto.png"] {
        assert!(!dir.join(gone).exists(), "{gone}");
    }
    assert!(dir.join("slot0.state").exists() && dir.join("slot9.state").exists());
}

#[test]
fn load_state_args_check_the_slot() {
    assert!(load_state_args(None).unwrap().is_empty());
    assert_eq!(
        load_state_args(Some(StateSlotId::Numbered(3))).unwrap(),
        ["--load-state", "3"]
    );
    assert_eq!(
        load_state_args(Some(StateSlotId::Auto)).unwrap(),
        ["--load-state", "auto"]
    );
    // Only reachable from code (deserialising rejects it), still refused.
    let out_of_range = StateSlotId::Numbered(MAX_STATE_SLOT + 1);
    assert_eq!(
        load_state_args(Some(out_of_range)).unwrap_err().kind,
        "invalid"
    );
    assert_eq!(validated(StateSlotId::Auto).unwrap(), StateSlotId::Auto);
    // IPC shapes: number or "auto"; anything else fails before the command runs.
    let parse = |s: &str| serde_json::from_str::<Option<StateSlotId>>(s);
    assert_eq!(parse("\"auto\"").unwrap(), Some(StateSlotId::Auto));
    assert_eq!(parse("null").unwrap(), None);
    assert!(parse("12").is_err() && parse("\"slot1\"").is_err());
}

#[test]
fn slots_the_runner_would_refuse_carry_the_reason() {
    let temp = scratch("strict");
    let dir = temp.path();
    write(dir, "slot0.state", b"x");
    write(dir, "slot0.json", &sidecar_json(1)); // core version 1.63
    write(dir, "slot1.state", b"x");
    let strict = StateLoadRule {
        strict_version: true,
        core_display_name: "Dolphin",
        installed_version: Some("1.64".into()),
        content_tag: None,
    };
    let slots = list_slots(dir, &strict);
    assert_eq!(
        slots[0].incompatible_reason.as_deref(),
        Some("Made by another Dolphin version")
    );
    assert_eq!(
        slots[1].incompatible_reason, None,
        "no sidecar: runner decides"
    );
    assert_eq!(
        list_slots(dir, &StateLoadRule::default())[0].incompatible_reason,
        None
    );
    let json = serde_json::to_value(&slots[0]).unwrap();
    assert_eq!(
        json["incompatibleReason"],
        "Made by another Dolphin version"
    );
}
