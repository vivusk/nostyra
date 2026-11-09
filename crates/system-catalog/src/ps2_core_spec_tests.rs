//! Final PCEE2 spec from the M18 spike: identity, host flags, fixed options (exact
//! strings, unique, `pcsx2_` prefix) and hardcore rules that match them.

use super::*;
use crate::{core, core_for_system, systems_for_core, FirmwareRule, CORES, PS2};

fn option(key: &str) -> Option<&'static str> {
    PCEE2
        .fixed_options
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}

#[test]
fn pcee2_is_the_registered_ps2_core() {
    assert_eq!(core("pcee2"), Some(&PCEE2));
    assert_eq!(core_for_system(PS2.id), Some(&PCEE2));
    assert_eq!(
        systems_for_core("pcee2")
            .iter()
            .map(|s| s.id)
            .collect::<Vec<_>>(),
        ["ps2"]
    );
    assert_eq!(CORES.len(), 14, "LRPS2 is not shipped (spike D1)");
    assert!(core("parallel_ps2").is_none());
    assert_eq!(PCEE2.library_stem, "pcee2_libretro");
    assert_eq!(PCEE2.display_name, "PCEE2");
    assert_eq!(PCEE2.license, "GPL-3.0-or-later");
}

#[test]
fn pcee2_host_flags_follow_the_spike() {
    // Through the registry lookup, so the flag asserts are not constant folds.
    let spec = core("pcee2").unwrap();
    assert_eq!(spec.savestates, SaveStateQuality::Serialized);
    assert_eq!(spec.max_state_bytes, 128 * 1024 * 1024, "96 MiB states");
    assert!(spec.exact_state_size, "size constant from before frame 1");
    assert!(!spec.requery_state_size, "no boot hold (spike D4)");
    assert!(spec.strict_state_version);
    assert!(!spec.hw_render_required, "readback fallback renders");
    assert!(
        spec.keep_library_loaded,
        "FreeLibrary after deinit crashes (dll_unloaded callback)"
    );
    assert!(!spec.stage_content);
    assert_eq!(spec.port_devices, [1, 1, 0, 0]);
    assert!(PCEE2.memory_cards.is_empty(), "shared cards under system/");
    assert!(PCEE2.firmware.is_empty() && PCEE2.bios_switch.is_none());
    assert_eq!(
        PCEE2.firmware_rule,
        FirmwareRule::Ps2Romdir {
            subdir: "pcsx2/bios",
            option: "pcsx2_bios"
        }
    );
    assert!(PCEE2.system_assets.is_none() && PCEE2.process_env.is_empty());
}

#[test]
fn pcee2_fixed_options_are_the_spike_strings() {
    assert_eq!(option("pcsx2_renderer"), Some("opengl"));
    assert_eq!(option("pcsx2_upscale_multiplier"), Some("1"));
    assert_eq!(option("pcsx2_skip_duplicate_frames"), Some("disabled"));
    assert_eq!(option("pcsx2_frame_limiter"), Some("frontend"));
    assert_eq!(option("pcsx2_fast_boot"), Some("enabled"));
    assert_eq!(option("pcsx2_cheats"), Some("disabled"));
    assert_eq!(option("pcsx2_ee_cycle_rate"), Some("0"));
    assert_eq!(option("pcsx2_ee_cycle_skip"), Some("0"));
    assert_eq!(option("pcsx2_widescreen_patches"), Some("disabled"));
    assert_eq!(option("pcsx2_no_interlacing_patches"), Some("disabled"));
    assert_eq!(option("pcsx2_rumble"), Some("disabled"));
    assert_eq!(option("pcsx2_multitap"), Some("disabled"));
    assert_eq!(option("pcsx2_lightgun"), Some("disabled"));
    assert_eq!(option("pcsx2_usb_mouse"), Some("disabled"));
    assert_eq!(PCEE2.fixed_options.len(), 14, "nothing else is fixed");
    assert_eq!(option("pcsx2_bios"), None, "answered per launch");
    assert_eq!(option("pcsx2_memcard_slot1_file"), None, "shared cards");
    for (i, (key, value)) in PCEE2.fixed_options.iter().enumerate() {
        assert!(key.starts_with("pcsx2_") && !value.is_empty(), "{key}");
        assert!(
            PCEE2.fixed_options[i + 1..].iter().all(|(o, _)| o != key),
            "{key}"
        );
    }
}

#[test]
fn pcee2_hardcore_rules_restate_the_fixed_options() {
    assert_eq!(PCEE2.hardcore_required_options, PCEE2_HARDCORE);
    assert_eq!(PCEE2_HARDCORE.len(), 5);
    for (key, value) in PCEE2_HARDCORE {
        assert_eq!(
            option(key),
            Some(*value),
            "{key}: same string on both sides"
        );
    }
    let keys: Vec<&str> = PCEE2_HARDCORE.iter().map(|(k, _)| *k).collect();
    assert!(!keys.contains(&"pcsx2_renderer") && !keys.contains(&"pcsx2_upscale_multiplier"));
}
