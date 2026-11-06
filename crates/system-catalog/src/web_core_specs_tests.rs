use super::*;
use crate::system_specs::ALL_SYSTEMS;

const MVP: [&str; 10] = [
    "nes", "snes", "gb", "gbc", "gba", "genesis", "sms", "gg", "sg1000", "psx",
];

#[test]
fn exactly_the_mvp_systems_are_on_the_web() {
    for spec in ALL_SYSTEMS {
        assert_eq!(
            spec.web_core.is_some(),
            MVP.contains(&spec.id),
            "{}",
            spec.id
        );
    }
    let core_of = |id: &str| system(id).and_then(|s| s.web_core).map(|w| w.ejs_core);
    assert_eq!(core_of("nes"), Some("fceumm"));
    assert_eq!(core_of("gb"), Some("mgba"), "mGBA forced, not Gambatte");
    assert_eq!(
        core_of("sms"),
        Some("genesis_plus_gx"),
        "GPGX forced, not SMS Plus"
    );
    assert_eq!(core_of("psx"), Some("pcsx_rearmed"));
}

#[test]
fn every_web_core_is_shipped_and_paired_or_web_only() {
    for spec in ALL_SYSTEMS {
        let Some(web) = spec.web_core else { continue };
        assert!(WEB_CORES.contains(&web), "{}", spec.id);
        let paired = SAME_LIBRETRO_CORE
            .iter()
            .any(|(ejs, _)| *ejs == web.ejs_core);
        assert_ne!(
            paired,
            WEB_ONLY_CORES.contains(&web.ejs_core),
            "{}: paired xor web-only",
            spec.id
        );
    }
}

#[test]
fn shared_core_layer_only_for_the_same_libretro_core() {
    assert_eq!(web_shared_core("psx").map(|c| c.id), Some("pcsx-rearmed"));
    assert_eq!(web_shared_core("snes").map(|c| c.id), Some("snes9x"));
    assert_eq!(web_shared_core("gbc").map(|c| c.id), Some("mgba"));
    assert_eq!(web_shared_core("gg").map(|c| c.id), Some("genesis_plus_gx"));
    assert_eq!(web_shared_core("nes"), None, "fceumm is not Mesen");
    assert_eq!(web_shared_core("n64"), None, "not on the web");
    assert_eq!(web_shared_core("nope"), None);
}

#[test]
fn ejs_save_folders_are_the_retroarch_core_names() {
    let dirs: Vec<&str> = WEB_CORES.iter().map(|c| c.save_dir).collect();
    assert_eq!(
        dirs,
        [
            "FCEUmm",
            "Snes9x",
            "mGBA",
            "Genesis Plus GX",
            "PCSX-ReARMed"
        ]
    );
}
