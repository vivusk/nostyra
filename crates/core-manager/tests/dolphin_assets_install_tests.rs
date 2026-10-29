//! Dolphin's system files install as shipped: a fake `Dolphin.zip` of the buildbot's
//! shape (top folder `dolphin-emu/`, markers two levels deep, thousands of small
//! entries — the real one holds 2,640 files) lands under `<cores>/dolphin-emu/`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::sync::atomic::AtomicBool;

use core_manager::install_core_with_assets;
use support::{fresh_dir, library_name, stub_probe_ok, zip_with, CORE_ID, LIBRARY_BYTES};
use system_catalog::DOLPHIN;

/// Entries under `dolphin-emu/Sys/`: the two markers plus `filler` game INIs.
fn dolphin_zip(filler: usize) -> Vec<u8> {
    let names: Vec<String> = (0..filler)
        .map(|i| format!("dolphin-emu/Sys/GameSettings/G{i:05}.ini"))
        .collect();
    let mut entries: Vec<(&str, &[u8])> = vec![
        ("dolphin-emu/Sys/codehandler.bin", b"codehandler"),
        ("dolphin-emu/Sys/GC/font_western.bin", b"font"),
        ("dolphin-emu/license.txt", b"GPL-2.0"),
    ];
    entries.extend(names.iter().map(|n| (n.as_str(), &b"[Core]"[..])));
    zip_with(&entries)
}

#[test]
fn dolphin_zip_shape_installs_with_nested_markers_and_many_entries() {
    let assets = DOLPHIN.system_assets.expect("Dolphin.zip");
    let temp = fresh_dir("assets-dolphin");
    let dir = temp.path();
    let core = zip_with(&[(&library_name(), LIBRARY_BYTES)]);
    let archive = dolphin_zip(2_700);
    let cancel = AtomicBool::new(false);
    install_core_with_assets(
        &core,
        Some((&assets, &archive)),
        CORE_ID,
        dir,
        stub_probe_ok,
        &cancel,
    )
    .unwrap();
    let root = dir.join(assets.dir);
    for marker in assets.markers {
        assert!(root.join(marker).is_file(), "{marker}");
    }
    assert!(root.join("Sys/GameSettings/G02699.ini").is_file());
    assert_eq!(
        std::fs::read_dir(root.join("Sys/GameSettings"))
            .unwrap()
            .count(),
        2_700
    );
}

#[test]
fn dolphin_zip_without_the_nested_font_marker_is_refused() {
    let assets = DOLPHIN.system_assets.expect("Dolphin.zip");
    let temp = fresh_dir("assets-dolphin-partial");
    let dir = temp.path();
    let core = zip_with(&[(&library_name(), LIBRARY_BYTES)]);
    let partial = zip_with(&[("dolphin-emu/Sys/codehandler.bin", b"codehandler")]);
    let cancel = AtomicBool::new(false);
    let result = install_core_with_assets(
        &core,
        Some((&assets, &partial)),
        CORE_ID,
        dir,
        stub_probe_ok,
        &cancel,
    );
    assert!(result.is_err(), "{result:?}");
    assert!(!dir.join(assets.dir).exists());
}
