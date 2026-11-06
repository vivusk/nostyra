//! The web edition reads content through a virtual source (no OS paths): the game
//! sha1 (and so the `[ID]` of the save folder) must equal the desktop one.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

use std::path::{Path, PathBuf};
use std::rc::Rc;

use content_file_source::{with_source, MemoryFiles};
use disc_image::test_images::{ps1_sectors, to_chd, to_raw, US_LICENCE};
use rom_identity::game_content_sha1;

fn test_rom(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../test-assets/roms")
        .join(name)
}

/// sha1 of `entry` read from the OS, and from memory under `/web/7/roms/` (the
/// `extra` files copied alongside, as the web scan pre-resolves cue siblings).
fn native_and_virtual(entry: &Path, extra: &[&Path], system: &str) -> (String, String) {
    let spec = system_catalog::system(system).unwrap();
    let native = game_content_sha1(entry, spec).unwrap();
    let virtual_path = |p: &Path| Path::new("/web/7/roms").join(p.file_name().unwrap());
    let mut files = MemoryFiles::new();
    for path in std::iter::once(&entry).chain(extra) {
        files.insert(virtual_path(path), std::fs::read(path).unwrap());
    }
    let web = with_source(Rc::new(files), || {
        game_content_sha1(&virtual_path(entry), spec).unwrap()
    });
    (native, web)
}

#[test]
fn cartridges_match_the_desktop_sha1() {
    for (name, system) in [
        ("nostyra-test.sfc", "snes"),
        ("nostyra-test.nes", "nes"),
        ("nostyra-test.gb", "gb"),
        ("nostyra-test.gba", "gba"),
        ("nostyra-test.gen", "genesis"),
        ("nostyra-test.sms", "sms"),
    ] {
        let (native, web) = native_and_virtual(&test_rom(name), &[], system);
        assert_eq!(native, web, "{name}");
    }
}

#[test]
fn cue_bin_and_m3u_match_through_their_referenced_files() {
    let cue = test_rom("nostyra-test-psx.cue");
    let bin = test_rom("nostyra-test-psx.bin");
    let (native, web) = native_and_virtual(&cue, &[&bin], "psx");
    assert_eq!(native, web);
    let m3u = test_rom("nostyra-test-psx.m3u");
    let disc2 = [
        bin.as_path(),
        &cue,
        &test_rom("nostyra-test-psx-disc2.cue"),
        &test_rom("nostyra-test-psx-disc2.bin"),
    ];
    let (native_m3u, web_m3u) = native_and_virtual(&m3u, &disc2, "psx");
    assert_eq!(native_m3u, web_m3u);
    assert_eq!(native_m3u, native, "a playlist is its first disc");
}

#[test]
fn chd_matches_the_desktop_sha1() {
    let dir = tempfile::tempdir().unwrap();
    let raw = to_raw(
        &ps1_sectors(Some(US_LICENCE), r"BOOT = cdrom:\SLUS_012.34;1"),
        2,
    );
    let chd = dir.path().join("game.chd");
    std::fs::write(&chd, to_chd(&raw, "MODE2_RAW")).unwrap();
    let (native, web) = native_and_virtual(&chd, &[], "psx");
    assert_eq!(native, web);
}
