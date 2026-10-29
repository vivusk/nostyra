//! PSP spec checks: shared disc extensions and probe order, pad, PPSSPP core spec.

use std::cell::RefCell;
use std::path::Path;

use super::*;
use crate::{classify, core_for_system, DiscProbe, SaveStateQuality, ALL_SYSTEMS, PPSSPP, PSX};

/// Records the signatures asked about; answers "no".
struct Refuse(RefCell<Vec<DiscSignature>>);

impl DiscProbe for Refuse {
    fn check(&self, signature: DiscSignature, _path: &Path) -> Result<(), String> {
        self.0.borrow_mut().push(signature);
        Err("no".into())
    }
}

#[test]
fn psp_shares_iso_chd_pbp_with_playstation_and_is_probed_last() {
    for ext in ["iso", "CSO", "chd", "pbp"] {
        assert!(
            PSP.has_extension(ext) && PSP.probe.checks_content(ext),
            "{ext}"
        );
    }
    for ext in ["elf", "prx", "zso", "m3u", "cue", "bin", "zip"] {
        assert!(!PSP.has_extension(ext), "{ext}");
    }
    for ext in ["iso", "chd", "pbp"] {
        assert!(PSX.has_extension(ext), "{ext} shared");
    }
    let disc: Vec<&str> = ALL_SYSTEMS
        .iter()
        .filter(|s| s.media == Media::Disc)
        .map(|s| s.id)
        .collect();
    assert_eq!(disc, ["psx", "dreamcast", "psp", "gc", "wii"]);
    use DiscSignature::{Dreamcast, GameCube, PlayStation, Psp, Wii};
    for (ext, asked) in [
        ("iso", &[PlayStation, Psp, GameCube, Wii][..]),
        ("chd", &[PlayStation, Dreamcast, Psp]),
        ("pbp", &[PlayStation, Psp]),
        ("cso", &[Psp]),
    ] {
        let probe = Refuse(RefCell::new(Vec::new()));
        assert!(classify(ext, 1_000_000, Path::new("x"), &probe).is_err());
        assert_eq!(*probe.0.borrow(), asked, "{ext}");
    }
    assert!(PSP.accepts_size(8 * 1024 * 1024 * 1024) && !PSP.accepts_size(15));
}

#[test]
fn psp_pad_is_the_retropad_with_face_symbols_and_one_stick() {
    assert_eq!(
        PSP.pad_labels[..4],
        [("b", "✕"), ("a", "○"), ("y", "□"), ("x", "△")]
    );
    assert!(PSP.pad_actions.contains(&"select") && PSP.pad_actions.contains(&"lstick_up"));
    assert!(!PSP
        .pad_actions
        .iter()
        .any(|a| a.starts_with("rstick") || a.ends_with('2')));
    assert_eq!(
        (PSP.analog_sticks, PSP.analog_triggers),
        (AnalogSticks::Both, false)
    );
}

#[test]
fn ppsspp_answers_every_key_the_spike_saw() {
    assert_eq!(core_for_system("psp").map(|c| c.id), Some("ppsspp"));
    let core = &PPSSPP;
    assert_eq!(core.fixed_options.len(), 70, "all keys of DLL 8a50821");
    let value = |key: &str| {
        core.fixed_options
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
    };
    for (key, want) in [
        ("ppsspp_cpu_core", "JIT"),
        ("ppsspp_internal_resolution", "960x544"),
        ("ppsspp_frame_duplication", "enabled"),
        ("ppsspp_frameskip", "disabled"),
        ("ppsspp_auto_frameskip", "disabled"),
        ("ppsspp_skip_buffer_effects", "disabled"),
        ("ppsspp_texture_replacement", "disabled"),
        ("ppsspp_backend", "auto"),
        ("ppsspp_inflight_frames", "Up to 1"),
        ("ppsspp_analog_deadzone", "0.15"),
        ("ppsspp_change_mac_address02", "2"),
    ] {
        assert_eq!(value(key), Some(want), "{key}");
    }
    assert_eq!(value("ppsspp_vr_mode"), None, "not in this build");
    let assets = core.system_assets.expect("PPSSPP.zip");
    assert_eq!((assets.dir, assets.markers[0]), ("PPSSPP", "compat.ini"));
    assert_eq!(core.process_env, [("CI", "1")]);
    assert!(core.requery_state_size && core.max_state_bytes == 128 * 1024 * 1024);
    assert_eq!(core.savestates, SaveStateQuality::Serialized);
    assert!(core.port_devices.is_empty() && core.firmware.is_empty());
}
