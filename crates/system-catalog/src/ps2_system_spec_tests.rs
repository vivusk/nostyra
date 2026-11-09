//! PS2 spec checks: registry position (right after PlayStation), shared disc
//! extensions and probe order, sizes, RA id, no web core, placeholder core.

use std::cell::RefCell;
use std::path::Path;

use super::*;
use crate::{
    candidates_for_extension, classify, core_for_system, DiscProbe, ALL_SYSTEMS, DISPLAY_ORDER,
    PCEE2, PSP, PSX,
};

/// Records the signatures asked about; answers "no".
struct Refuse(RefCell<Vec<DiscSignature>>);

impl DiscProbe for Refuse {
    fn check(&self, signature: DiscSignature, _path: &Path) -> Result<(), String> {
        self.0.borrow_mut().push(signature);
        Err("no".into())
    }
}

#[test]
fn ps2_follows_playstation_and_precedes_dreamcast() {
    assert_eq!(ALL_SYSTEMS.len(), 20);
    assert_eq!(DISPLAY_ORDER.len(), 20);
    let ids: Vec<&str> = ALL_SYSTEMS.iter().map(|s| s.id).collect();
    let psx = ids.iter().position(|id| *id == "psx").unwrap();
    assert_eq!(ids[psx..psx + 3], ["psx", "ps2", "dreamcast"]);
    let shown: Vec<&str> = DISPLAY_ORDER.iter().map(|s| s.id).collect();
    let psx = shown.iter().position(|id| *id == "psx").unwrap();
    assert_eq!(shown[psx..psx + 3], ["psx", "ps2", "psp"]);
    assert_eq!(crate::system("ps2"), Some(&PS2));
    assert_eq!(
        (PS2.display_name, PS2.short_label),
        ("PlayStation 2", "PS2")
    );
}

#[test]
fn ps2_shares_every_extension_and_is_probed_after_playstation() {
    assert_eq!(PS2.media, Media::Disc);
    for ext in ["iso", "BIN", "cue", "chd", "cso", "ZSO", "m3u"] {
        assert!(
            PS2.has_extension(ext) && PS2.probe.checks_content(ext),
            "{ext}"
        );
    }
    for ext in ["pbp", "gdi", "cdi", "elf", "gz", "zip"] {
        assert!(!PS2.has_extension(ext), "{ext}");
    }
    assert!(!PSX.has_extension("zso") && !PSP.has_extension("zso"));
    let ids =
        |ext: &str| -> Vec<&str> { candidates_for_extension(ext).iter().map(|s| s.id).collect() };
    assert_eq!(ids("zso"), ["ps2"]);
    assert_eq!(ids("cso"), ["ps2", "psp"]);
    assert_eq!(ids("iso"), ["psx", "ps2", "psp", "gc", "wii"]);
    assert_eq!(ids("cue"), ["psx", "ps2", "dreamcast"]);
    use DiscSignature::{Dreamcast, PlayStation, PlayStation2, Psp};
    // A `.bin` past the Mega Drive / Atari sizes: only the two disc probes run.
    for (ext, asked) in [
        ("bin", &[PlayStation, PlayStation2][..]),
        ("chd", &[PlayStation, PlayStation2, Dreamcast, Psp]),
        ("zso", &[PlayStation2]),
    ] {
        let probe = Refuse(RefCell::new(Vec::new()));
        assert!(classify(ext, 600_000_000, Path::new("x"), &probe).is_err());
        assert_eq!(*probe.0.borrow(), asked, "{ext}");
    }
}

#[test]
fn ps2_sizes_cover_a_cue_sheet_and_a_dual_layer_dvd() {
    assert!(PS2.accepts_size(16) && !PS2.accepts_size(15));
    assert!(PS2.accepts_size(8_500_000_000));
    assert!(PS2.accepts_size(9 * 1024 * 1024 * 1024));
    assert!(!PS2.accepts_size(9 * 1024 * 1024 * 1024 + 1));
}

/// The label PS2 gives `action`, if any.
fn label(action: &str) -> Option<&'static str> {
    PS2.pad_labels
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, l)| *l)
}

/// DualShock 2 = the PS1 set + L3 / R3 + both sticks; every action is listed once
/// and carries the console's own button name (PS1 itself shows RetroPad names).
#[test]
fn ps2_pad_is_the_dualshock_2_with_sony_labels() {
    assert_eq!(&PS2.pad_actions[..14], PSX.pad_actions, "PS1 set first");
    #[rustfmt::skip]
    let extras: &[&str] = &[
        "l3", "r3",
        "lstick_up", "lstick_down", "lstick_left", "lstick_right",
        "rstick_up", "rstick_down", "rstick_left", "rstick_right",
    ];
    assert_eq!(&PS2.pad_actions[14..], extras);
    assert_eq!(PS2.pad_actions.len(), 24, "every RetroPad action");
    let dpad = ["up", "down", "left", "right"];
    for (i, action) in PS2.pad_actions.iter().enumerate() {
        assert!(!PS2.pad_actions[i + 1..].contains(action), "{action}");
        assert_eq!(label(action).is_some(), !dpad.contains(action), "{action}");
    }
    assert_eq!(PS2.pad_labels.len(), PS2.pad_actions.len() - dpad.len());
    assert!(PSX.pad_labels.is_empty(), "PS1 keeps RetroPad names");
    for (action, want) in [
        ("b", "Cross"),
        ("a", "Circle"),
        ("y", "Square"),
        ("x", "Triangle"),
        ("l", "L1"),
        ("r", "R1"),
        ("l2", "L2"),
        ("r2", "R2"),
        ("l3", "L3"),
        ("r3", "R3"),
        ("select", "Select"),
        ("start", "Start"),
        ("lstick_up", "Left Stick Up"),
        ("rstick_right", "Right Stick Right"),
    ] {
        assert_eq!(label(action), Some(want), "{action}");
    }
    assert_eq!(
        (PS2.analog_sticks, PS2.analog_triggers),
        (AnalogSticks::Both, false),
        "two sticks; L2 / R2 digital (pressure not exposed)"
    );
    assert!(PS2.touch_screen.is_none());
}

#[test]
fn ps2_ra_id_data_names_and_web_core() {
    assert_eq!(PS2.ra_console_id, Some(21));
    assert!(PS2.web_core.is_none(), "EmulatorJS has no PS2 core");
    assert_eq!(PS2.dat_file, PS2.metadat_file);
    assert_eq!(PS2.thumbnails_folder.replace(' ', "_"), PS2.thumbnails_repo);
}

/// The spec itself is checked in `ps2_core_spec_tests`.
#[test]
fn pcee2_is_the_default_core_and_ps2_states_are_large() {
    assert_eq!(core_for_system("ps2").map(|c| c.id), Some("pcee2"));
    assert_eq!(PCEE2.library_stem, "pcee2_libretro");
    assert_eq!(PCEE2.systems, ["ps2"]);
    assert!(PCEE2.license.starts_with("GPL-3.0"));
    assert!(crate::has_large_states("ps2"), "96 MiB resume points");
}
