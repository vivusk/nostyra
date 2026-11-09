//! Registry tests for the Atari systems and cores (`atari_system_specs`,
//! `atari_core_specs`).

use crate::per_extension_probe::ExtCheck;
use crate::{
    core, core_for_system, ContentProbe, HeaderRule, ALL_SYSTEMS, ATARI2600, ATARI7800,
    DISPLAY_ORDER, LYNX,
};

const ATARI: [&crate::SystemSpec; 3] = [&ATARI2600, &ATARI7800, &LYNX];

#[test]
fn atari_systems_follow_sg1000_and_game_gear() {
    let ids: Vec<&str> = ALL_SYSTEMS.iter().map(|s| s.id).collect();
    let sg = ids.iter().position(|id| *id == "sg1000").unwrap();
    assert_eq!(ids[sg + 1..sg + 4], ["atari2600", "atari7800", "lynx"]);
    assert_eq!(ids[sg + 4], "psx", "cartridges before discs");
    let shown: Vec<&str> = DISPLAY_ORDER.iter().map(|s| s.id).collect();
    let gg = shown.iter().position(|id| *id == "gg").unwrap();
    assert_eq!(shown[gg + 1..], ["atari2600", "atari7800", "lynx"]);
    assert_eq!(ALL_SYSTEMS.len(), 20);
}

#[test]
fn every_extension_is_in_exactly_one_check_group() {
    for spec in ATARI {
        let ContentProbe::PerExtension(groups) = spec.probe else {
            panic!("{}: per-extension probe", spec.id);
        };
        for ext in spec.extensions {
            let hits = groups.iter().filter(|g| g.exts.contains(ext)).count();
            assert_eq!(hits, 1, "{}: {ext}", spec.id);
        }
        for group in groups {
            assert!(group.exts.iter().all(|e| spec.extensions.contains(e)));
        }
    }
}

#[test]
fn only_bin_is_dat_gated() {
    for spec in ATARI {
        for ext in spec.extensions {
            assert_eq!(
                spec.probe.is_dat_gated(ext),
                *ext == "bin",
                "{}: {ext}",
                spec.id
            );
        }
    }
    assert!(!LYNX.probe.is_quiet("lnx") && LYNX.probe.is_quiet("o") && LYNX.probe.is_quiet("bll"));
    let ContentProbe::PerExtension(groups) = ATARI7800.probe else {
        unreachable!()
    };
    assert!(groups.iter().any(|g| g.check == ExtCheck::DatHash));
}

#[test]
fn headered_systems_strip_their_header_for_the_fallback_hash() {
    assert_eq!(ATARI2600.header_rule, HeaderRule::None);
    for (spec, len) in [(&ATARI7800, 128), (&LYNX, 64)] {
        let HeaderRule::MagicHeader { len: got, .. } = spec.header_rule else {
            panic!("{}", spec.id);
        };
        assert_eq!(got, len, "{}", spec.id);
        assert_eq!(spec.max_bytes, 1024 * 1024 + len as u64);
    }
}

#[test]
fn each_atari_system_has_its_own_core() {
    for (system, core_id, stem) in [
        ("atari2600", "stella", "stella_libretro"),
        ("atari7800", "prosystem", "prosystem_libretro"),
        ("lynx", "handy", "handy_libretro"),
    ] {
        let spec = core_for_system(system).unwrap();
        assert_eq!((spec.id, spec.library_stem), (core_id, stem));
        assert_eq!(spec.systems, [system]);
        assert!(spec.firmware.is_empty(), "no BIOS entries");
    }
    assert_eq!(
        core("handy").unwrap().fixed_options,
        [("handy_refresh_rate", "60")]
    );
    assert!(core("stella").unwrap().fixed_options.is_empty());
    assert!(core("prosystem").unwrap().fixed_options.is_empty());
}

#[test]
fn thumbnail_and_dat_names() {
    for (spec, name) in [(&ATARI2600, "2600"), (&ATARI7800, "7800"), (&LYNX, "Lynx")] {
        assert_eq!(spec.dat_file, format!("Atari - {name}.dat"));
        assert_eq!(spec.metadat_file, spec.dat_file);
        assert_eq!(spec.thumbnails_repo, format!("Atari_-_{name}"));
        assert_eq!(spec.thumbnails_folder, format!("Atari - {name}"));
    }
}

#[test]
fn pads_name_the_console_buttons_and_lynx_has_no_select() {
    for spec in ATARI {
        for (action, _) in spec.pad_labels {
            let listed = spec.pad_actions.iter().filter(|a| *a == action).count();
            assert_eq!(
                listed, 1,
                "{}: label {action} names one listed action",
                spec.id
            );
        }
    }
    let label = |spec: &crate::SystemSpec, action: &str| {
        spec.pad_labels
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, l)| *l)
    };
    assert_eq!(label(&ATARI2600, "start"), Some("Game Reset"));
    assert_eq!(label(&ATARI2600, "l2"), Some("Left difficulty B"));
    assert_eq!(label(&ATARI7800, "x"), Some("Reset"));
    assert_eq!(label(&LYNX, "l"), Some("Option 1"));
    assert!(
        !LYNX.pad_actions.contains(&"select"),
        "Handy rotates on SELECT"
    );
    assert_eq!(ATARI2600.pad_actions.len(), 13);
    assert_eq!(ATARI7800.pad_actions.len(), 11);
    assert_eq!(LYNX.pad_actions.len(), 9);
}
