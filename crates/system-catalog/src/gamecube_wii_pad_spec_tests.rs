//! GameCube / Wii pad checks: the Dolphin RetroPad maps (GC pad; Wii Remote +
//! Nunchuk in IR mode 2), their Controls labels, and which pad extras each uses.

use super::*;

/// The label a system gives `action`, if any.
fn label(spec: &SystemSpec, action: &str) -> Option<&'static str> {
    spec.pad_labels
        .iter()
        .find(|(a, _)| *a == action)
        .map(|(_, l)| *l)
}

/// The four stick directions of `prefix` (`lstick` / `rstick`).
fn stick(prefix: &str) -> [String; 4] {
    ["up", "down", "left", "right"].map(|dir| format!("{prefix}_{dir}"))
}

#[test]
fn every_label_names_a_listed_action_once() {
    for spec in [GC, WII] {
        for (i, (action, text)) in spec.pad_labels.iter().enumerate() {
            assert!(spec.pad_actions.contains(action), "{}: {action}", spec.id);
            assert!(!text.is_empty(), "{}: {action}", spec.id);
            let rest = &spec.pad_labels[i + 1..];
            assert!(
                rest.iter().all(|(a, _)| a != action),
                "{}: {action}",
                spec.id
            );
        }
        for (i, action) in spec.pad_actions.iter().enumerate() {
            assert!(!spec.pad_actions[i + 1..].contains(action), "{action}");
        }
        for action in stick("lstick").iter().chain(&stick("rstick")) {
            assert!(spec.pad_actions.contains(&action.as_str()), "{action}");
        }
    }
}

#[test]
fn gamecube_pad_has_z_analog_triggers_light_press_and_c_stick() {
    #[rustfmt::skip]
    let want: &[&str] = &[
        "a", "b", "x", "y", "start", "up", "down", "left", "right", "r", "l2", "r2", "l3", "r3",
        "lstick_up", "lstick_down", "lstick_left", "lstick_right",
        "rstick_up", "rstick_down", "rstick_left", "rstick_right",
    ];
    assert_eq!(GC.pad_actions, want);
    assert!(!GC.pad_actions.contains(&"select") && !GC.pad_actions.contains(&"l"));
    for (action, want) in [
        ("a", "A"),
        ("b", "B"),
        ("x", "X"),
        ("y", "Y"),
        ("r", "Z"),
        ("l2", "L"),
        ("r2", "R"),
        ("l3", "L (light press)"),
        ("r3", "R (light press)"),
        ("lstick_left", "Stick Left"),
        ("rstick_up", "C-Stick Up"),
        ("rstick_right", "C-Stick Right"),
    ] {
        assert_eq!(label(&GC, action), Some(want), "{action}");
    }
}

#[test]
fn wii_pad_is_remote_and_nunchuk_with_tilt_on_the_right_stick() {
    #[rustfmt::skip]
    let want: &[&str] = &[
        "a", "b", "x", "y", "select", "start", "up", "down", "left", "right", "l", "r", "l2", "r2",
        "r3",
        "lstick_up", "lstick_down", "lstick_left", "lstick_right",
        "rstick_up", "rstick_down", "rstick_left", "rstick_right",
    ];
    assert_eq!(WII.pad_actions, want);
    assert!(!WII.pad_actions.contains(&"l3"), "sideways toggle disabled");
    for (action, want) in [
        ("a", "A"),
        ("b", "B"),
        ("x", "Nunchuk C"),
        ("y", "Nunchuk Z"),
        ("start", "1"),
        ("select", "2"),
        ("l", "\u{2212}"),
        ("r", "+"),
        ("l2", "Shake Nunchuk"),
        ("r2", "Shake Remote"),
        ("r3", "Home"),
        ("lstick_up", "Nunchuk Stick Up"),
        ("rstick_down", "Tilt Remote Down"),
        ("rstick_left", "Tilt Remote Left"),
    ] {
        assert_eq!(label(&WII, action), Some(want), "{action}");
    }
}
