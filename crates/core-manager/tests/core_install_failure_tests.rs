//! Offline failure-path tests: a failed probe, cancel, an oversized entry or a
//! locked target must leave no new core file and no `*.tmp` behind.

#![allow(clippy::unwrap_used, clippy::expect_used)] // test code: panics are the failure signal

mod support;

use std::io::Write;

use core_manager::{installed_core, CoreManagerError, MAX_CORE_BYTES};
use support::{
    assert_empty, fresh_dir, install, library_name, stub_probe_ok, zip_with, CORE_ID, LIBRARY_BYTES,
};
use zip::write::SimpleFileOptions;

#[test]
fn failing_probe_leaves_no_core_and_no_tmp() {
    let temp = fresh_dir("probe-fail");
    let dir = temp.path();
    let archive = zip_with(&[(&library_name(), LIBRARY_BYTES)]);
    let result = install(&archive, dir, |_| Err("not a libretro core".into()), false);
    assert!(matches!(result, Err(CoreManagerError::ProbeFailed(m)) if m.contains("libretro")));
    assert_empty(dir);
    assert_eq!(installed_core(dir, CORE_ID), None);
}

#[test]
fn failing_probe_keeps_previous_install() {
    let temp = fresh_dir("probe-fail-keep");
    let dir = temp.path();
    let lib = library_name();
    std::fs::write(dir.join(&lib), b"working build").unwrap();
    let archive = zip_with(&[(&lib, LIBRARY_BYTES)]);
    assert!(install(&archive, dir, |_| Err("bad".into()), false).is_err());
    assert_eq!(std::fs::read(dir.join(&lib)).unwrap(), b"working build");
    assert!(!dir.join(format!("{lib}.tmp")).exists());
}

#[test]
fn cancel_leaves_no_core() {
    let temp = fresh_dir("cancel");
    let dir = temp.path();
    let archive = zip_with(&[(&library_name(), LIBRARY_BYTES)]);
    let result = install(&archive, dir, stub_probe_ok, true);
    assert!(matches!(result, Err(CoreManagerError::Cancelled)));
    assert_empty(dir);
}

#[test]
fn oversized_library_is_rejected_and_cleaned_up() {
    let temp = fresh_dir("too-large");
    let dir = temp.path();
    // Zeros deflate to a tiny archive: a zip bomb in miniature.
    let huge = vec![0u8; usize::try_from(MAX_CORE_BYTES).unwrap() + 1];
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    writer
        .start_file(library_name(), SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&huge).unwrap();
    let archive = writer.finish().unwrap().into_inner();
    drop(huge);

    let result = install(&archive, dir, stub_probe_ok, false);

    assert!(matches!(result, Err(CoreManagerError::TooLarge { .. })));
    assert_empty(dir);
}

#[cfg(windows)]
#[test]
fn locked_core_maps_to_core_in_use() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = fresh_dir("in-use");
    let dir = temp.path();
    let lib = library_name();
    std::fs::write(dir.join(&lib), b"loaded by a runner").unwrap();
    // No sharing at all, like a DLL mapped by a running process.
    let _lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(dir.join(&lib))
        .unwrap();

    let archive = zip_with(&[(&lib, LIBRARY_BYTES)]);
    let error = install(&archive, dir, stub_probe_ok, false).unwrap_err();

    assert!(
        matches!(error, CoreManagerError::CoreInUse { .. }),
        "{error:?}"
    );
    assert!(error.to_string().contains("in use"));
    assert!(!dir.join(format!("{lib}.tmp")).exists());
}
