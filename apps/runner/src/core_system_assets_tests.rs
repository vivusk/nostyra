//! `ensure_core_assets` on temp folders with a fake core spec.

use std::path::Path;

use system_catalog::{CoreAssets, CoreSpec, CORE_DEFAULTS};

use super::*;
use crate::runner_exit_codes::{exit_code_for, EXIT_CORE_LOAD};

const FAKE: CoreSpec = CoreSpec {
    id: "fake",
    display_name: "Fake",
    system_assets: Some(CoreAssets {
        dir: "FAKE",
        markers: &["compat.ini", "atlas.zim"],
        archive_path: "assets/system/FAKE.zip",
    }),
    ..CORE_DEFAULTS
};

/// (`data/system`, `cores`) under a fresh temp dir.
fn layout() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-core-assets-")
        .tempdir()
        .unwrap();
    let (system, cores) = (
        temp.path().join("data/system"),
        temp.path().join("data/cores"),
    );
    std::fs::create_dir_all(&system).unwrap();
    std::fs::create_dir_all(&cores).unwrap();
    (temp, system, cores)
}

fn put(dir: &Path, compat: &str) {
    std::fs::create_dir_all(dir.join("flash0/font")).unwrap();
    std::fs::write(dir.join("compat.ini"), compat).unwrap();
    std::fs::write(dir.join("atlas.zim"), "zim").unwrap();
    std::fs::write(dir.join("flash0/font/a.pgf"), "font").unwrap();
}

fn ensure(system: &Path, cores: &Path) -> Result<Option<AssetsState>, MissingCoreAssets> {
    ensure_core_assets(&FAKE, system, cores)
}

#[test]
fn core_without_assets_needs_nothing() {
    let (_temp, system, cores) = layout();
    assert_eq!(
        ensure_core_assets(&system_catalog::SNES9X, &system, &cores).unwrap(),
        None
    );
    assert_eq!(std::fs::read_dir(&system).unwrap().count(), 0);
}

#[test]
fn copy_beside_the_core_is_installed_with_subfolders() {
    let (_temp, system, cores) = layout();
    put(&cores.join("FAKE"), "v1");
    assert_eq!(
        ensure(&system, &cores).unwrap(),
        Some(AssetsState::Installed)
    );
    assert!(system.join("FAKE/flash0/font/a.pgf").is_file());
    let (tmp, old) = staging_dirs(&system.join("FAKE"));
    assert!(!tmp.exists() && !old.exists());
    assert_eq!(ensure(&system, &cores).unwrap(), Some(AssetsState::Present));
}

#[test]
fn installed_copy_with_the_same_content_is_kept() {
    let (_temp, system, cores) = layout();
    put(&cores.join("FAKE"), "v1");
    put(&system.join("FAKE"), "v1");
    std::fs::write(system.join("FAKE/user-extra.txt"), "x").unwrap();
    assert_eq!(ensure(&system, &cores).unwrap(), Some(AssetsState::Present));
    assert!(system.join("FAKE/user-extra.txt").exists(), "no re-copy");
}

#[test]
fn newer_files_beside_the_core_are_copied_again() {
    let (_temp, system, cores) = layout();
    put(&system.join("FAKE"), "v1");
    put(&cores.join("FAKE"), "v2 (core update)");
    assert_eq!(
        ensure(&system, &cores).unwrap(),
        Some(AssetsState::Installed)
    );
    let compat = std::fs::read_to_string(system.join("FAKE/compat.ini")).unwrap();
    assert_eq!(compat, "v2 (core update)");
}

#[test]
fn partial_install_and_leftovers_are_replaced() {
    let (_temp, system, cores) = layout();
    put(&cores.join("FAKE"), "v1");
    std::fs::create_dir_all(system.join("FAKE")).unwrap();
    std::fs::write(system.join("FAKE/compat.ini"), "v1").unwrap(); // no atlas.zim
    std::fs::create_dir_all(system.join("FAKE.tmp")).unwrap();
    std::fs::create_dir_all(system.join("FAKE.old")).unwrap();
    assert_eq!(
        ensure(&system, &cores).unwrap(),
        Some(AssetsState::Installed)
    );
    assert!(system.join("FAKE/atlas.zim").is_file());
    assert!(!system.join("FAKE.tmp").exists() && !system.join("FAKE.old").exists());
}

#[test]
fn installed_copy_works_without_one_beside_the_core() {
    let (_temp, system, cores) = layout();
    put(&system.join("FAKE"), "v1");
    assert_eq!(ensure(&system, &cores).unwrap(), Some(AssetsState::Present));
}

#[test]
fn nothing_anywhere_is_exit_3_with_the_redownload_hint() {
    let (_temp, system, cores) = layout();
    let err = ensure(&system, &cores).unwrap_err();
    let text = err.to_string();
    assert!(
        text.starts_with("Fake system files are missing ("),
        "{text}"
    );
    assert!(
        text.contains(&system.join("FAKE").display().to_string()),
        "{text}"
    );
    assert!(
        text.ends_with("Re-download the core in Settings → Cores."),
        "{text}"
    );
    let chained = anyhow::Error::from(err).context("cannot start");
    assert_eq!(exit_code_for(&chained), EXIT_CORE_LOAD);
}

#[test]
fn cores_dir_is_the_library_folder_in_both_layouts() {
    let launcher = Path::new("C:/Users/x/AppData/Roaming/Nostyra/data/cores/ppsspp_libretro.dll");
    assert_eq!(
        cores_dir_of(launcher),
        Path::new("C:/Users/x/AppData/Roaming/Nostyra/data/cores")
    );
    assert_eq!(
        cores_dir_of(Path::new("cores/ppsspp_libretro.so")),
        Path::new("cores")
    );
    assert_eq!(cores_dir_of(Path::new("ppsspp_libretro.so")), Path::new(""));
}
