//! Dreamcast spec checks: shared disc extensions, registry order, pad, core.

use super::*;
use crate::{core_for_system, ALL_SYSTEMS, PSX};

#[test]
fn dreamcast_shares_disc_extensions_with_playstation_but_not_bin() {
    let dc = DREAMCAST;
    for ext in ["gdi", "CDI", "chd", "cue", "m3u"] {
        assert!(
            dc.has_extension(ext) && dc.probe.checks_content(ext),
            "{ext}"
        );
    }
    for ext in ["bin", "iso", "elf", "zip", "pbp"] {
        assert!(!dc.has_extension(ext), "{ext}");
    }
    for ext in ["cue", "chd", "m3u"] {
        assert!(PSX.has_extension(ext), "{ext} shared");
    }
    let disc: Vec<&str> = ALL_SYSTEMS
        .iter()
        .filter(|s| s.media == Media::Disc)
        .map(|s| s.id)
        .collect();
    assert_eq!(
        disc,
        ["psx", "dreamcast", "psp", "gc", "wii"],
        "discs last, PlayStation first"
    );
    assert_eq!(dc.pad_labels[0], ("b", "A"));
    assert!(dc.pad_actions.contains(&"r2") && !dc.pad_actions.contains(&"select"));
    assert_eq!(core_for_system("dreamcast").map(|c| c.id), Some("flycast"));
}
