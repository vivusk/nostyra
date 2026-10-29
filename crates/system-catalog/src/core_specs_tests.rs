//! Registry consistency and lookups for `core_specs`.

use super::*;

#[test]
fn registry_is_consistent() {
    for system in ALL_SYSTEMS {
        let core = core(system.default_core_id).expect("default core exists");
        assert!(
            core.systems.contains(&system.id),
            "{} lists {}",
            core.id,
            system.id
        );
    }
    for (i, core) in CORES.iter().enumerate() {
        assert!(CORES[i + 1..].iter().all(|other| other.id != core.id));
        for id in core.systems {
            assert!(
                system(id).is_some(),
                "{} names unknown system {id}",
                core.id
            );
        }
    }
}

#[test]
fn lookups() {
    assert_eq!(core_for_system("snes").map(|c| c.id), Some("snes9x"));
    assert_eq!(core_for_system("nes").map(|c| c.id), Some("mesen"));
    assert_eq!(core_for_system("gba").map(|c| c.id), Some("mgba"));
    assert_eq!(core_for_system("psx").map(|c| c.id), Some("pcsx-rearmed"));
    assert_eq!(
        core_for_system("genesis").map(|c| c.id),
        Some("genesis_plus_gx")
    );
    assert_eq!(
        core_for_system("n64").map(|c| c.id),
        Some("mupen64plus-next")
    );
    assert_eq!(core_for_system("no-such-system"), None);
    assert_eq!(
        systems_for_core("mupen64plus-next")
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        ["n64"]
    );
    assert_eq!(
        systems_for_core("mgba")
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        ["gb", "gbc", "gba"]
    );
    assert_eq!(
        systems_for_core("mesen")
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        ["nes"]
    );
    assert_eq!(
        systems_for_core("genesis_plus_gx")
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        ["genesis", "sms", "gg", "sg1000"]
    );
    assert!(systems_for_core("nope").is_empty());
}

#[test]
fn memory_cards_only_for_disc_cores() {
    for core in [
        SNES9X,
        MESEN,
        MGBA,
        GENESIS_PLUS_GX,
        MUPEN64PLUS_NEXT,
        FLYCAST,
    ] {
        assert!(core.memory_cards.is_empty(), "{}", core.id);
    }
    assert_eq!(
        PCSX_REARMED.memory_cards,
        [
            MemoryCardSlot::SaveRam,
            MemoryCardSlot::CoreFile("pcsx-card2.mcd")
        ]
    );
}

#[test]
fn state_quality_port_devices_and_exact_size_per_core() {
    use SaveStateQuality::{Deterministic, Serialized};
    for core in CORES {
        // (savestates, port devices set, exact state size)
        let expected = match core.id {
            "mupen64plus-next" => (Serialized, false, true),
            "flycast" => (Serialized, true, false),
            "ppsspp" => (Serialized, false, false),
            "melonds_ds" => (Deterministic, false, true),
            "dolphin-emu" => (Serialized, true, false),
            _ => (Deterministic, false, false),
        };
        let actual = (
            core.savestates,
            !core.port_devices.is_empty(),
            core.exact_state_size,
        );
        assert_eq!(actual, expected, "{}", core.id);
    }
    assert_eq!(
        FLYCAST.port_devices,
        [1, 1, 0, 0],
        "all four ports, pads on 1-2"
    );
}

#[test]
fn flycast_fixed_options_are_complete_and_safe() {
    let value = |key: &str| {
        FLYCAST
            .fixed_options
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    };
    assert_eq!(value("reicast_internal_resolution"), Some("1280x960"));
    assert_eq!(value("reicast_threaded_rendering"), Some("disabled"));
    assert_eq!(value("reicast_per_content_vmus"), Some("All VMUs"));
    assert_eq!(value("reicast_enable_dsp"), Some("enabled"));
    assert_eq!(value("reicast_analog_stick_deadzone"), Some("15%"));
    assert_ne!(value("reicast_alpha_sorting"), Some("per-pixel (accurate)"));
    for port in 1..=4 {
        let key = format!("reicast_device_port{port}_slot2");
        assert_eq!(value(&key), Some("None"), "no rumble pack yet");
    }
    let options = FLYCAST.fixed_options;
    for (i, (key, _)) in options.iter().enumerate() {
        assert!(key.starts_with("reicast_"), "{key}");
        assert!(options[i + 1..].iter().all(|(o, _)| o != key), "{key}");
    }
}

#[test]
fn mupen_fixed_options_cover_the_mandatory_keys() {
    let value = |key: &str| {
        MUPEN64PLUS_NEXT
            .fixed_options
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    };
    assert_eq!(value("mupen64plus-cpucore"), Some("dynamic_recompiler"));
    assert_eq!(value("mupen64plus-43screensize"), Some("960x720"));
    assert_eq!(value("mupen64plus-ThreadedRenderer"), Some("False"));
    assert_eq!(value("mupen64plus-astick-sensitivity"), Some("100"));
    assert_eq!(value("mupen64plus-pak1"), Some("memory"));
    // 0 (unanswered) corrupts the heap in real games.
    assert_eq!(value("mupen64plus-MaxTxCacheSize"), Some("8000"));
    let options = MUPEN64PLUS_NEXT.fixed_options;
    for (i, (key, _)) in options.iter().enumerate() {
        assert!(key.starts_with("mupen64plus-"), "{key}");
        assert!(
            options[i + 1..].iter().all(|(other, _)| other != key),
            "{key}"
        );
    }
}

#[test]
fn per_core_host_rules_are_well_formed() {
    for core in CORES {
        assert!(
            core.max_state_bytes >= DEFAULT_MAX_STATE_BYTES,
            "{}",
            core.id
        );
        if let Some(assets) = core.system_assets {
            assert!(
                !assets.dir.is_empty() && !assets.markers.is_empty(),
                "{}",
                core.id
            );
            assert!(assets.archive_path.ends_with(".zip"), "{}", core.id);
        }
        assert!(core
            .process_env
            .iter()
            .all(|(k, _)| !k.is_empty() && !k.contains('=')));
    }
    // Cores without the new needs keep the defaults.
    assert_eq!(SNES9X.max_state_bytes, DEFAULT_MAX_STATE_BYTES);
    assert!(!FLYCAST.requery_state_size && FLYCAST.system_assets.is_none());
    assert!(MUPEN64PLUS_NEXT.process_env.is_empty());
}
