//! Core system files installed with the core (`install_core_with_assets`), offline:
//! in-memory zips, a fake assets spec, the snes9x library name.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::sync::atomic::AtomicBool;

use core_manager::{install_core_with_assets, CoreManagerError};
use support::{fresh_dir, library_name, stub_probe_ok, zip_with, CORE_ID, LIBRARY_BYTES};
use system_catalog::CoreAssets;

const ASSETS: CoreAssets = CoreAssets {
    dir: "FAKE",
    markers: &["compat.ini", "atlas.zim"],
    archive_path: "assets/system/FAKE.zip",
};

fn good_assets() -> Vec<u8> {
    zip_with(&[
        ("FAKE/compat.ini", b"[compat]"),
        ("FAKE/atlas.zim", b"zim"),
        ("FAKE/flash0/font/a.pgf", b"font"),
    ])
}

fn install(
    dir: &std::path::Path,
    assets: Option<&[u8]>,
) -> Result<core_manager::InstalledCore, CoreManagerError> {
    let core = zip_with(&[(&library_name(), LIBRARY_BYTES)]);
    let assets = assets.map(|bytes| (&ASSETS, bytes));
    let cancel = AtomicBool::new(false);
    install_core_with_assets(&core, assets, CORE_ID, dir, stub_probe_ok, &cancel)
}

#[test]
fn every_entry_lands_in_the_assets_folder_beside_the_core() {
    let temp = fresh_dir("assets-ok");
    let dir = temp.path();
    install(dir, Some(&good_assets())).unwrap();
    assert!(dir.join(library_name()).is_file());
    assert_eq!(
        std::fs::read(dir.join("FAKE/compat.ini")).unwrap(),
        b"[compat]"
    );
    assert!(dir.join("FAKE/flash0/font/a.pgf").is_file());
    assert!(!dir.join("FAKE.tmp").exists() && !dir.join("FAKE.old").exists());
}

#[test]
fn reinstall_replaces_older_assets() {
    let temp = fresh_dir("assets-replace");
    let dir = temp.path();
    install(dir, Some(&good_assets())).unwrap();
    std::fs::write(dir.join("FAKE/stale.txt"), b"old").unwrap();
    install(dir, Some(&good_assets())).unwrap();
    assert!(
        !dir.join("FAKE/stale.txt").exists(),
        "whole folder replaced"
    );
}

#[test]
fn unsafe_entry_fails_the_whole_install_and_keeps_the_old_library() {
    let temp = fresh_dir("assets-slip");
    let dir = temp.path();
    std::fs::write(dir.join(library_name()), b"old core").unwrap();
    let evil = zip_with(&[("FAKE/compat.ini", b"x"), ("../evil.txt", b"x")]);
    let result = install(dir, Some(&evil));
    assert!(
        matches!(result, Err(CoreManagerError::AssetsArchive(_))),
        "{result:?}"
    );
    assert_eq!(
        std::fs::read(dir.join(library_name())).unwrap(),
        b"old core"
    );
    assert!(!dir.join("FAKE.tmp").exists() && !dir.join("FAKE").exists());
    assert!(!temp.path().parent().unwrap().join("evil.txt").exists());
}

#[test]
fn missing_marker_fails_before_the_library_swap() {
    let temp = fresh_dir("assets-marker");
    let dir = temp.path();
    let partial = zip_with(&[("FAKE/compat.ini", b"x")]);
    let result = install(dir, Some(&partial));
    assert!(
        matches!(result, Err(CoreManagerError::AssetsArchive(_))),
        "{result:?}"
    );
    assert!(
        !dir.join(library_name()).exists(),
        "library never installed"
    );
    assert!(!dir.join("FAKE.tmp").exists());
}

#[test]
fn oversized_assets_are_refused() {
    let temp = fresh_dir("assets-big");
    let dir = temp.path();
    let huge = vec![0u8; usize::try_from(core_manager::MAX_ASSETS_BYTES).unwrap() + 1];
    let bomb = zip_with(&[("FAKE/compat.ini", b"x"), ("FAKE/big.bin", &huge)]);
    drop(huge);
    let result = install(dir, Some(&bomb));
    assert!(
        matches!(result, Err(CoreManagerError::TooLarge { .. })),
        "{result:?}"
    );
    assert!(!dir.join("FAKE.tmp").exists() && !dir.join(library_name()).exists());
}

#[test]
fn failed_library_install_discards_the_staged_assets() {
    let temp = fresh_dir("assets-probe");
    let dir = temp.path();
    let core = zip_with(&[(&library_name(), LIBRARY_BYTES)]);
    let assets = good_assets();
    let cancel = AtomicBool::new(false);
    let result = install_core_with_assets(
        &core,
        Some((&ASSETS, &assets)),
        CORE_ID,
        dir,
        |_| Err("not a core".into()),
        &cancel,
    );
    assert!(matches!(result, Err(CoreManagerError::ProbeFailed(_))));
    assert!(!dir.join("FAKE").exists() && !dir.join("FAKE.tmp").exists());
}

#[test]
fn core_without_assets_installs_as_before() {
    let temp = fresh_dir("assets-none");
    let dir = temp.path();
    install(dir, None).unwrap();
    let names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names, [std::ffi::OsString::from(library_name())]);
}
