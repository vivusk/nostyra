//! Shipped fixed options pass hardcore; known cheats / slowdowns do not.

use system_catalog::{core, CORES};

use super::*;

/// `library_name` each pinned core reports (M15 spike step 6; Dolphin: rcheevos' key).
const LIBRARY_NAMES: [(&str, &str); 10] = [
    ("snes9x", "Snes9x"),
    ("mesen", "Mesen"),
    ("mgba", "mGBA"),
    ("pcsx-rearmed", "PCSX-ReARMed"),
    ("genesis_plus_gx", "Genesis Plus GX"),
    ("mupen64plus-next", "Mupen64Plus-Next"),
    ("flycast", "Flycast"),
    ("ppsspp", "PPSSPP"),
    ("melonds_ds", "melonDS DS"),
    ("dolphin-emu", "dolphin-emu"),
];

fn library_name(core_id: &str) -> &'static str {
    LIBRARY_NAMES
        .iter()
        .find(|(id, _)| *id == core_id)
        .map(|(_, name)| *name)
        .unwrap_or_else(|| panic!("no library name for core {core_id}"))
}

#[test]
fn every_fixed_option_of_every_core_is_allowed() {
    assert_eq!(CORES.len(), LIBRARY_NAMES.len());
    for spec in CORES {
        let name = library_name(spec.id);
        for (key, value) in spec.fixed_options {
            assert!(
                !is_option_disallowed(name, spec, key, value),
                "{}: {key}={value}",
                spec.id
            );
        }
    }
}

#[test]
fn rcheevos_table_rules_apply_by_library_name() {
    let snes9x = core("snes9x").unwrap();
    assert!(is_option_disallowed(
        "Snes9x",
        snes9x,
        "snes9x_region",
        "pal"
    ));
    assert!(is_option_disallowed(
        "Snes9x",
        snes9x,
        "snes9x_layer_1",
        "disabled"
    ));
    assert!(!is_option_disallowed(
        "Snes9x",
        snes9x,
        "snes9x_region",
        "ntsc"
    ));
    let pcsx = core("pcsx-rearmed").unwrap();
    assert!(is_option_disallowed(
        "PCSX-ReARMed",
        pcsx,
        "pcsx_rearmed_psxclock",
        "50"
    ));
    assert!(!is_option_disallowed(
        "PCSX-ReARMed",
        pcsx,
        "pcsx_rearmed_psxclock",
        "auto"
    ));
    assert!(!is_option_disallowed(
        "Unknown Core",
        pcsx,
        "pcsx_rearmed_region",
        "pal"
    ));
}

#[test]
fn melonds_ds_must_stay_in_ds_mode() {
    let melonds = core("melonds_ds").unwrap();
    let key = melonds.hardcore_required_options[0].0;
    assert!(is_option_disallowed("melonDS DS", melonds, key, "dsi"));
    assert!(is_option_disallowed("melonDS DS", melonds, key, "auto"));
    assert!(!is_option_disallowed("melonDS DS", melonds, key, "DS"));
}
