//! Shared helpers for the offline install tests: temp dirs, in-memory zip
//! fixtures built with the `zip` writer, and stub probes.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)] // test code: panics are the failure signal

use std::io::Write;
use std::path::Path;
use std::sync::atomic::AtomicBool;

use core_manager::{
    current_platform_core_source, install_core_from_archive, CoreInfo, CoreManagerError,
    InstalledCore,
};
use zip::write::SimpleFileOptions;

pub const CORE_ID: &str = "snes9x";
pub const LIBRARY_BYTES: &[u8] = b"pretend this is a libretro core";

pub fn library_name() -> String {
    current_platform_core_source(CORE_ID)
        .unwrap()
        .library_file_name
}

/// Fresh, empty per-test directory under the system temp dir; removed on drop.
pub fn fresh_dir(test: &str) -> tempfile::TempDir {
    let prefix = format!("nostyra-core-manager-{test}-");
    tempfile::Builder::new().prefix(&prefix).tempdir().unwrap()
}

pub fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

pub fn stub_probe_ok(path: &Path) -> Result<CoreInfo, String> {
    assert!(
        path.to_string_lossy().ends_with(".tmp"),
        "probe must see the tmp file"
    );
    Ok(CoreInfo {
        name: "Snes9x".into(),
        version: "1.63".into(),
    })
}

pub fn install(
    archive: &[u8],
    dir: &Path,
    probe: impl FnOnce(&Path) -> Result<CoreInfo, String>,
    cancel: bool,
) -> Result<InstalledCore, CoreManagerError> {
    install_core_from_archive(archive, CORE_ID, dir, probe, &AtomicBool::new(cancel))
}

/// The cores dir holds nothing at all (no library, no tmp).
pub fn assert_empty(dir: &Path) {
    let names: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(names.is_empty(), "unexpected files: {names:?}");
}
