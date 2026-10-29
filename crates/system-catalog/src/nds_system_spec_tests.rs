//! NDS spec checks: extensions, sizes, touch rule, pad, melonDS DS core spec.

use super::*;
use crate::{
    candidates_for_extension, core_for_system, system, AnalogSticks, SaveStateQuality, ALL_SYSTEMS,
    MELONDS_DS, NINTENDO_64,
};

#[test]
fn nds_is_a_cartridge_system_after_the_nintendo_64() {
    assert_eq!(system("nds"), Some(&NDS));
    let order: Vec<&str> = ALL_SYSTEMS.iter().map(|s| s.id).collect();
    let n64 = order.iter().position(|id| *id == NINTENDO_64.id).unwrap();
    assert_eq!(order[n64 + 1], "nds");
    assert_eq!(NDS.media, Media::Cartridge);
    assert_eq!(NDS.header_rule, HeaderRule::None);
    for ext in ["nds", "DSI", "ids"] {
        let ids: Vec<&str> = candidates_for_extension(ext).iter().map(|s| s.id).collect();
        assert_eq!(ids, ["nds"], "{ext} belongs to nobody else");
        assert!(NDS.probe.checks_content(ext) && !NDS.probe.is_quiet(ext));
    }
    for ext in ["srl", "bcn", "bin", "zip", "7z"] {
        assert!(!NDS.has_extension(ext), "{ext}");
    }
    assert!(NDS.accepts_size(0x1000) && !NDS.accepts_size(0xFFF));
    assert!(NDS.accepts_size(512 * 1024 * 1024) && !NDS.accepts_size(512 * 1024 * 1024 + 1));
}

#[test]
fn nds_data_names_and_touch_rule() {
    assert_eq!(NDS.dat_file, "Nintendo - Nintendo DS.dat");
    assert_eq!(NDS.metadat_file, NDS.dat_file);
    assert_eq!(NDS.thumbnails_repo, "Nintendo_-_Nintendo_DS");
    assert_eq!(NDS.thumbnails_folder, "Nintendo - Nintendo DS");
    assert_eq!(NDS.touch_screen, Some(TouchScreenRule::DsBottomScreen));
    let pointers: Vec<&str> = ALL_SYSTEMS
        .iter()
        .filter(|s| s.touch_screen.is_some())
        .map(|s| s.id)
        .collect();
    assert_eq!(pointers, ["nds", "wii"], "DS touch screen, Wii Remote IR");
    let aiming: Vec<&str> = ALL_SYSTEMS
        .iter()
        .filter(|s| s.touch_screen.is_some_and(TouchScreenRule::aims))
        .map(|s| s.id)
        .collect();
    assert_eq!(aiming, ["wii"], "only the Wii aims (DS touch unchanged)");
}

#[test]
fn nds_pad_has_touch_mic_swap_and_the_right_stick_cursor() {
    assert_eq!(
        NDS.pad_actions,
        [
            "b",
            "a",
            "y",
            "x",
            "select",
            "start",
            "up",
            "down",
            "left",
            "right",
            "l",
            "r",
            "r2",
            "l3",
            "r3",
            "rstick_up",
            "rstick_down",
            "rstick_left",
            "rstick_right",
        ]
    );
    assert!(!NDS.pad_actions.iter().any(|a| a.starts_with("lstick")));
    assert!(
        !NDS.pad_actions.contains(&"l2"),
        "lid not exposed; LT stays rewind"
    );
    let label = |action: &str| NDS.pad_labels.iter().find(|(a, _)| *a == action);
    assert_eq!(label("r2"), Some(&("r2", "Touch (stick cursor)")));
    assert_eq!(label("l3"), Some(&("l3", "Microphone (blow)")));
    assert_eq!(label("r3"), Some(&("r3", "Swap screens")));
    for (action, _) in NDS.pad_labels {
        assert!(NDS.pad_actions.contains(action), "{action}");
    }
}

#[test]
fn only_the_ds_reads_the_right_stick_alone() {
    for spec in ALL_SYSTEMS {
        let want = match spec.id {
            "nds" => AnalogSticks::RightOnly,
            "n64" | "dreamcast" | "psp" | "gc" | "wii" => AnalogSticks::Both,
            _ => AnalogSticks::None,
        };
        assert_eq!(spec.analog_sticks, want, "{}", spec.id);
    }
}

#[test]
fn melonds_ds_core_spec_follows_the_spike() {
    let core = core_for_system("nds").expect("default core");
    assert_eq!(core, &MELONDS_DS);
    assert_eq!(core.id, "melonds_ds", "core_layer_id(\"melonDS DS\")");
    assert_eq!(core.library_stem, "melondsds_libretro");
    assert_eq!(core.systems, ["nds"]);
    assert_eq!(core.savestates, SaveStateQuality::Deterministic);
    assert!(core.exact_state_size && !core.requery_state_size);
    assert_eq!(core.max_state_bytes, 128 * 1024 * 1024);
    assert_eq!(core.process_env, [("MELONDSDS_SKIP_ERROR_SCREEN", "1")]);
    assert!(core.firmware.is_empty() && core.memory_cards.is_empty());
    assert!(core.bios_switch.is_none() && core.system_assets.is_none());
}

#[test]
fn melonds_options_are_the_verified_set() {
    let options = MELONDS_DS.fixed_options;
    assert_eq!(options.len(), 44);
    for (i, (key, _)) in options.iter().enumerate() {
        assert!(key.starts_with("melonds_"), "{key}");
        assert!(options[i + 1..].iter().all(|(k, _)| k != key), "{key}");
    }
    let value = |key: &str| options.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
    for (key, want) in [
        ("melonds_console_mode", "ds"),
        ("melonds_boot_mode", "direct"),
        ("melonds_sysfile_mode", "builtin"),
        ("melonds_firmware_nds_path", "/notfound"),
        ("melonds_homebrew_sdcard", "disabled"),
        ("melonds_dsi_sdcard", "disabled"),
        ("melonds_render_mode", "software"),
        ("melonds_threaded_renderer", "enabled"),
        ("melonds_screen_layout1", "top-bottom"),
        ("melonds_screen_layout2", "left-right"),
        ("melonds_number_of_screen_layouts", "2"),
        ("melonds_show_cursor", "touching"),
        ("melonds_touch_mode", "auto"),
        ("melonds_mic_input", "blow"),
        ("melonds_firmware_username", "melonDS DS"),
        ("melonds_network_mode", "disabled"),
        ("melonds_start_time_mode", "real"),
    ] {
        assert_eq!(value(key), Some(want), "{key}");
    }
    // `absolute` would need all six date keys (else the core aborts).
    assert!(!options
        .iter()
        .any(|(k, _)| k.contains("start_time_absolute")));
    for (key, v) in options
        .iter()
        .filter(|(k, _)| k.starts_with("melonds_show_"))
    {
        assert!(*v == "disabled" || *key == "melonds_show_cursor", "{key}");
    }
}
