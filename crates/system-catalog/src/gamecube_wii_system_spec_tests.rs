//! GameCube / Wii spec checks: extensions, probe order, sizes, data names, analog
//! fields, Wii pointer, and the Dolphin core spec (assets, options, the three
//! runner flags). Pad actions / labels: `gamecube_wii_pad_spec_tests.rs`.

use std::cell::RefCell;
use std::path::Path;

use super::*;
use crate::{
    candidates_for_extension, classify, core_for_system, system, DiscProbe, SaveStateQuality,
    ALL_SYSTEMS, CORES, DOLPHIN,
};

/// Records the signatures asked about; answers "no".
struct Refuse(RefCell<Vec<DiscSignature>>);

impl DiscProbe for Refuse {
    fn check(&self, signature: DiscSignature, _path: &Path) -> Result<(), String> {
        self.0.borrow_mut().push(signature);
        Err(format!("{signature:?}"))
    }
}

#[test]
fn gc_and_wii_are_the_last_disc_systems_and_probe_every_extension() {
    assert_eq!(system("gc"), Some(&GC));
    assert_eq!(system("wii"), Some(&WII));
    let order: Vec<&str> = ALL_SYSTEMS.iter().map(|s| s.id).collect();
    assert_eq!(order[order.len() - 3..], ["psp", "gc", "wii"]);
    for spec in [GC, WII] {
        assert_eq!(spec.media, Media::Disc);
        assert_eq!(spec.header_rule, HeaderRule::None);
        for ext in spec.extensions {
            assert!(spec.probe.checks_content(ext), "{}: {ext}", spec.id);
        }
        for ext in ["wad", "tgc", "nkit", "chd", "zip", "7z", "cso", "bin"] {
            assert!(!spec.has_extension(ext), "{}: {ext}", spec.id);
        }
    }
    assert!(GC.has_extension("GCM") && GC.has_extension("m3u") && !GC.has_extension("wbfs"));
    assert!(WII.has_extension("WBFS") && !WII.has_extension("gcm") && !WII.has_extension("m3u"));
    let ids =
        |ext: &str| -> Vec<&str> { candidates_for_extension(ext).iter().map(|s| s.id).collect() };
    assert_eq!(ids("iso"), ["psx", "psp", "gc", "wii"]);
    assert_eq!(ids("m3u"), ["psx", "dreamcast", "gc"]);
    assert_eq!(ids("rvz"), ["gc", "wii"]);
    assert_eq!(ids("wbfs"), ["wii"]);
    use DiscSignature::{GameCube, PlayStation, Psp, Wii};
    for (ext, asked) in [
        ("iso", &[PlayStation, Psp, GameCube, Wii][..]),
        ("dol", &[GameCube, Wii]),
        ("wbfs", &[Wii]),
    ] {
        let probe = Refuse(RefCell::new(Vec::new()));
        let err = classify(ext, 1_000_000, Path::new("x"), &probe).unwrap_err();
        assert_eq!(*probe.0.borrow(), asked, "{ext}");
        // The last candidate's reject text is the scan-problem text.
        assert_eq!(err.to_string(), format!("{:?}", asked[asked.len() - 1]));
    }
}

#[test]
fn sizes_cover_mini_dvd_and_dual_layer_discs() {
    assert!(GC.accepts_size(16) && !GC.accepts_size(15));
    assert!(GC.accepts_size(1_459_978_240) && GC.accepts_size(2 * 1024 * 1024 * 1024));
    assert!(!GC.accepts_size(2 * 1024 * 1024 * 1024 + 1));
    assert!(WII.accepts_size(8_511_160_320) && !WII.accepts_size(8 * 1024 * 1024 * 1024 + 1));
}

#[test]
fn data_names_analog_fields_and_the_wii_pointer() {
    assert_eq!(GC.dat_file, "Nintendo - GameCube.dat");
    assert_eq!(WII.dat_file, "Nintendo - Wii.dat");
    assert_eq!(
        (GC.thumbnails_repo, GC.thumbnails_folder),
        ("Nintendo_-_GameCube", "Nintendo - GameCube")
    );
    assert_eq!(
        (WII.thumbnails_repo, WII.thumbnails_folder),
        ("Nintendo_-_Wii", "Nintendo - Wii")
    );
    for spec in [GC, WII] {
        assert_eq!(spec.metadat_file, spec.dat_file);
        assert_eq!(spec.analog_sticks, AnalogSticks::Both);
        assert_eq!(spec.default_core_id, DOLPHIN.id);
    }
    assert_eq!((GC.analog_triggers, WII.analog_triggers), (true, false));
    assert_eq!((GC.short_label, WII.short_label), ("GC", "Wii"));
    assert_eq!(GC.touch_screen, None, "no pointer on the GameCube");
    assert_eq!(WII.touch_screen, Some(TouchScreenRule::WiiRemoteIr));
}

#[test]
fn dolphin_core_spec_follows_the_spike() {
    assert_eq!(core_for_system("gc"), Some(&DOLPHIN));
    assert_eq!(core_for_system("wii"), Some(&DOLPHIN));
    // `core_layer_id("dolphin-emu")` keeps the dash.
    assert_eq!(
        (DOLPHIN.id, DOLPHIN.library_stem),
        ("dolphin-emu", "dolphin_libretro")
    );
    assert_eq!(DOLPHIN.systems, ["gc", "wii"]);
    assert_eq!(DOLPHIN.savestates, SaveStateQuality::Serialized);
    assert_eq!(
        (DOLPHIN.requery_state_size, DOLPHIN.exact_state_size),
        (true, false)
    );
    assert_eq!(DOLPHIN.max_state_bytes, 256 * 1024 * 1024);
    assert_eq!(DOLPHIN.port_devices, [1, 1, 0, 0]);
    assert!(DOLPHIN.firmware.is_empty() && DOLPHIN.bios_switch.is_none());
    assert!(DOLPHIN.memory_cards.is_empty() && DOLPHIN.process_env.is_empty());
    let assets = DOLPHIN.system_assets.expect("Dolphin.zip");
    assert_eq!(assets.dir, "dolphin-emu");
    assert_eq!(
        assets.markers,
        ["Sys/codehandler.bin", "Sys/GC/font_western.bin"]
    );
    assert_eq!(assets.archive_path, "assets/system/Dolphin.zip");
    for core in CORES {
        let flags = (
            core.hw_render_required,
            core.strict_state_version,
            core.keep_library_loaded,
        );
        let want = core.id == DOLPHIN.id;
        assert_eq!(flags, (want, want, want), "{}", core.id);
    }
}

#[test]
fn dolphin_options_are_the_spike_verified_values() {
    let options = DOLPHIN.fixed_options;
    assert_eq!(options.len(), 30);
    for (i, (key, _)) in options.iter().enumerate() {
        assert!(key.starts_with("dolphin_"), "{key}");
        assert!(options[i + 1..].iter().all(|(k, _)| k != key), "{key}");
    }
    let value = |key: &str| options.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
    for (key, want) in [
        ("dolphin_main_cpu_thread", "disabled"),
        ("dolphin_renderer", "Hardware"),
        ("dolphin_efb_scale", "1"),
        ("dolphin_shader_compilation_mode", "0"),
        ("dolphin_osd_enabled", "disabled"),
        ("dolphin_cheats_import", "disabled"),
        ("dolphin_skip_gc_bios", "enabled"),
        ("dolphin_hotkey_sideways_toggle", "Disabled"),
        ("dolphin_call_back_audio_method", "1"),
        ("dolphin_log_level", "3"),
        ("dolphin_wiimote_default_device", "nunchuk"),
        ("dolphin_ir_mode", "2"),
        ("dolphin_enable_rumble", "disabled"),
        ("dolphin_gc_sp1", "255"),
    ] {
        assert_eq!(value(key), Some(want), "{key}");
    }
    assert_eq!(value("dolphin_cpu_core"), None, "per-arch declared default");
}
