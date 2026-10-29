//! Registry invariants and per-system field checks for `system_specs`.

use super::*;
use crate::{DREAMCAST, NINTENDO_64, PROBE_BYTES};

#[test]
fn snes_extensions_are_case_insensitive() {
    assert!(SNES.matches_extension(Path::new("a/Game.SFC")));
    assert!(SNES.matches_extension(Path::new("game.smc")));
    assert!(!SNES.matches_extension(Path::new("game.zip")));
    assert!(!SNES.matches_extension(Path::new("sfc")));
    assert!(NES.matches_extension(Path::new("x/Mario.NES")));
    assert!(GB.matches_extension(Path::new("x/Tetris.GB")));
    assert!(!GB.matches_extension(Path::new("x/zelda.gbc")));
    assert!(GBC.matches_extension(Path::new("x/zelda.gbc")));
    assert!(GBA.matches_extension(Path::new("x/Metroid.GBA")));
}

#[test]
fn size_ranges() {
    assert!(!SNES.accepts_size(32 * 1024 - 1));
    assert!(SNES.accepts_size(32 * 1024));
    assert!(SNES.accepts_size(16 * 1024 * 1024));
    assert!(!SNES.accepts_size(16 * 1024 * 1024 + 1));
    assert!(!NES.accepts_size(8 * 1024 - 1));
    assert!(NES.accepts_size(16 + 16 * 1024 + 8 * 1024));
    assert!(!NES.accepts_size(8 * 1024 * 1024 + 1));
    for spec in [GB, GBC] {
        assert!(!spec.accepts_size(32 * 1024 - 1));
        assert!(spec.accepts_size(32 * 1024));
        assert!(spec.accepts_size(8 * 1024 * 1024));
        assert!(!spec.accepts_size(8 * 1024 * 1024 + 1));
    }
    assert!(!GBA.accepts_size(1023));
    assert!(GBA.accepts_size(1024));
    assert!(GBA.accepts_size(32 * 1024 * 1024));
    assert!(!GBA.accepts_size(32 * 1024 * 1024 + 1));
}

#[test]
fn lookup_by_id() {
    assert_eq!(system("snes"), Some(&SNES));
    assert_eq!(system("nes"), Some(&NES));
    assert_eq!(system("gb"), Some(&GB));
    assert_eq!(system("gbc"), Some(&GBC));
    assert_eq!(system("gba"), Some(&GBA));
    assert_eq!(system("psx"), Some(&PSX));
    assert_eq!(system("genesis"), Some(&GENESIS));
    assert_eq!(system("sms"), Some(&SMS));
    assert_eq!(system("gg"), Some(&GG));
    assert_eq!(system("sg1000"), Some(&SG1000));
    assert_eq!(system("n64"), Some(&NINTENDO_64));
    assert_eq!(system("no-such-system"), None);
}

#[test]
fn display_order_lists_every_system_once() {
    let mut shown: Vec<&str> = DISPLAY_ORDER.iter().map(|s| s.id).collect();
    let mut registry: Vec<&str> = ALL_SYSTEMS.iter().map(|s| s.id).collect();
    shown.sort_unstable();
    registry.sort_unstable();
    assert_eq!(shown, registry);
}

#[test]
fn ids_are_unique_and_extensions_unambiguous() {
    for (i, a) in ALL_SYSTEMS.iter().enumerate() {
        for b in &ALL_SYSTEMS[i + 1..] {
            assert_ne!(a.id, b.id);
            // Guard: a shared extension needs a probe on every candidate.
            for ext in a.extensions {
                assert!(
                    !b.has_extension(ext)
                        || (a.probe.checks_content(ext) && b.probe.checks_content(ext)),
                    "{ext} shared by {} and {} without content probes",
                    a.id,
                    b.id
                );
            }
        }
    }
}

#[test]
fn probes_fit_in_probe_bytes() {
    for spec in ALL_SYSTEMS {
        let ends: Vec<usize> = match spec.probe {
            ContentProbe::Magic { offset, bytes, .. } => vec![offset + bytes.len()],
            ContentProbe::MagicForExtensions { offsets, bytes, .. } => {
                offsets.iter().map(|offset| offset + bytes.len()).collect()
            }
            ContentProbe::MagicAny {
                offset, patterns, ..
            } => patterns.iter().map(|bytes| offset + bytes.len()).collect(),
            ContentProbe::Any | ContentProbe::Disc(_) | ContentProbe::NdsHeader => continue,
        };
        assert!(ends.iter().all(|&end| end <= PROBE_BYTES), "{}", spec.id);
    }
}

#[test]
fn extensions_are_lower_case() {
    for spec in ALL_SYSTEMS {
        for ext in spec.extensions {
            assert_eq!(*ext, ext.to_ascii_lowercase());
        }
    }
}

#[test]
fn handheld_pads() {
    assert_eq!(GB.pad_actions.len(), 8);
    assert_eq!(GBC.pad_actions, GB.pad_actions);
    assert_eq!(GBA.pad_actions.len(), 10);
    assert_eq!(&GBA.pad_actions[8..], ["l", "r"]);
}

#[test]
fn disc_extensions_always_probe() {
    for spec in ALL_SYSTEMS.iter().filter(|s| s.media == Media::Disc) {
        assert!(
            matches!(spec.probe, ContentProbe::Disc(_)),
            "{}: disc systems decide by content (.bin/.iso are shared names)",
            spec.id
        );
    }
    assert_eq!(PSX.media, Media::Disc);
    assert!(PSX.accepts_size(16) && !PSX.accepts_size(15));
    assert!(PSX.accepts_size(4 * 1024 * 1024 * 1024));
    for cart in [
        SNES,
        NES,
        GB,
        GBC,
        GBA,
        NINTENDO_64,
        GENESIS,
        SMS,
        GG,
        SG1000,
    ] {
        assert_eq!(cart.media, Media::Cartridge, "{}", cart.id);
    }
}

#[test]
fn sega_pads_and_labels() {
    assert_eq!(GENESIS.pad_actions.len(), 12);
    for spec in [SMS, GG, SG1000] {
        assert_eq!(
            spec.pad_actions,
            ["b", "a", "start", "up", "down", "left", "right"]
        );
    }
    assert_eq!(GENESIS.pad_labels[0], ("y", "A"));
    assert_eq!(SMS.pad_labels.last(), Some(&("start", "Pause")));
    for spec in ALL_SYSTEMS {
        for (action, label) in spec.pad_labels {
            assert!(spec.pad_actions.contains(action), "{}: {action}", spec.id);
            assert!(!label.is_empty(), "{}: {action}", spec.id);
        }
    }
    for spec in [SNES, NES, GB, GBC, GBA, PSX] {
        assert!(spec.pad_labels.is_empty(), "{}", spec.id);
    }
}

#[test]
fn genesis_checks_content_only_on_bin_and_md() {
    assert!(GENESIS.probe.checks_content("bin") && GENESIS.probe.checks_content("MD"));
    assert!(!GENESIS.probe.checks_content("gen") && !GENESIS.probe.checks_content("smd"));
    assert!(GENESIS.probe.is_quiet("md") && !GENESIS.probe.is_quiet("bin"));
    assert!(GENESIS.accepts_size(512) && !GENESIS.accepts_size(511));
    assert!(GENESIS.accepts_size(10 * 1024 * 1024) && !GENESIS.accepts_size(10 * 1024 * 1024 + 1));
    for spec in [SMS, GG, SG1000] {
        assert!(
            spec.accepts_size(1024) && !spec.accepts_size(1023),
            "{}",
            spec.id
        );
        assert!(!spec.accepts_size(4 * 1024 * 1024 + 1), "{}", spec.id);
    }
}

#[test]
fn nintendo_64_pad_sizes_and_analog() {
    let n64 = NINTENDO_64;
    assert!(n64.accepts_size(4 * 1024) && !n64.accepts_size(4 * 1024 - 1));
    assert!(n64.accepts_size(64 * 1024 * 1024) && !n64.accepts_size(64 * 1024 * 1024 + 1));
    for ext in ["z64", "N64", "v64"] {
        assert!(
            n64.probe.checks_content(ext) && !n64.probe.is_quiet(ext),
            "{ext}"
        );
    }
    assert_eq!(
        n64.pad_actions[..10],
        ["b", "y", "l2", "l", "r", "start", "up", "down", "left", "right"]
    );
    assert_eq!(n64.pad_actions.len(), 18, "+ 8 stick directions");
    assert_eq!(n64.pad_labels[2], ("l2", "Z"));
    assert!(n64.pad_labels.contains(&("rstick_up", "C-Up")));
    assert!(n64.pad_labels.contains(&("lstick_left", "Stick Left")));
    for spec in ALL_SYSTEMS {
        let triggers = spec.id == DREAMCAST.id || spec.id == crate::GC.id;
        assert_eq!(spec.analog_triggers, triggers, "{}", spec.id);
    }
}
