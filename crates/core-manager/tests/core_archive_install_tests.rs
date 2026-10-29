//! Offline happy-path and archive-content tests of the zip → tmp → probe →
//! atomic install pipeline, using archives built in memory.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use core_manager::{installed_core, CoreManagerError};
use sha2::{Digest, Sha256};
use support::{
    assert_empty, fresh_dir, install, library_name, stub_probe_ok, zip_with, CORE_ID, LIBRARY_BYTES,
};

#[test]
fn probe_ok_installs_library_with_correct_sha256() {
    let temp = fresh_dir("ok");
    let dir = temp.path();
    let lib = library_name();
    let archive = zip_with(&[("readme.txt", b"hi"), (&lib, LIBRARY_BYTES)]);
    assert_eq!(installed_core(dir, CORE_ID), None);

    let core = install(&archive, dir, stub_probe_ok, false).unwrap();

    assert_eq!(core.path, dir.join(&lib));
    assert_eq!(std::fs::read(&core.path).unwrap(), LIBRARY_BYTES);
    let expected: String = Sha256::digest(LIBRARY_BYTES)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(core.sha256, expected);
    assert_eq!(core.info.name, "Snes9x");
    assert_eq!(installed_core(dir, CORE_ID), Some(dir.join(&lib)));
    assert!(!dir.join(format!("{lib}.tmp")).exists());
    assert!(
        !dir.join("readme.txt").exists(),
        "only the library is extracted"
    );
}

#[test]
fn already_installed_check_ignores_unknown_cores_and_tmp_files() {
    let temp = fresh_dir("installed-check");
    let dir = temp.path();
    std::fs::write(dir.join(format!("{}.tmp", library_name())), b"partial").unwrap();
    assert_eq!(installed_core(dir, CORE_ID), None);
    assert_eq!(installed_core(dir, "unknown-core"), None);
}

#[test]
fn reinstall_replaces_existing_core() {
    let temp = fresh_dir("reinstall");
    let dir = temp.path();
    let lib = library_name();
    std::fs::write(dir.join(&lib), b"old build").unwrap();
    let archive = zip_with(&[(&lib, LIBRARY_BYTES)]);
    install(&archive, dir, stub_probe_ok, false).unwrap();
    assert_eq!(std::fs::read(dir.join(&lib)).unwrap(), LIBRARY_BYTES);
}

#[test]
fn creates_missing_cores_dir() {
    let temp = fresh_dir("create-dir");
    let dir = temp.path().join("cores");
    let archive = zip_with(&[(&library_name(), LIBRARY_BYTES)]);
    install(&archive, &dir, stub_probe_ok, false).unwrap();
    assert!(installed_core(&dir, CORE_ID).is_some());
}

#[test]
fn other_and_traversal_entry_names_are_ignored() {
    let temp = fresh_dir("traversal");
    let dir = temp.path();
    let lib = library_name();
    let nested = format!("nested/{lib}");
    let traversal = format!("../{lib}");
    let archive = zip_with(&[(&nested, b"x"), (&traversal, b"evil"), ("other.dll", b"y")]);

    let result = install(&archive, dir, stub_probe_ok, false);

    assert!(matches!(result, Err(CoreManagerError::LibraryNotInArchive(name)) if name == lib));
    assert_empty(dir);
}

#[test]
fn garbage_bytes_are_an_invalid_archive() {
    let temp = fresh_dir("garbage");
    let dir = temp.path();
    let result = install(b"not a zip at all", dir, stub_probe_ok, false);
    assert!(matches!(result, Err(CoreManagerError::InvalidArchive(_))));
}
