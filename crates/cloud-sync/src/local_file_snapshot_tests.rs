use std::path::Path;

use super::*;
use crate::content_hash::sha256_hex;
use crate::sync_manifest::{FileEntry, RemoteManifest};

fn write(dir: &Path, rel: &str, bytes: &[u8]) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// The base a sync would record for `snapshot` (shas + stats).
fn base_of(snapshot: &LocalSnapshot) -> BaseManifest {
    let mut manifest = RemoteManifest::empty("dev", "PC");
    let mut local = BTreeMap::new();
    for (path, file) in &snapshot.files {
        let Some(sha) = &file.sha256 else { continue };
        let entry = FileEntry {
            sha256: sha.clone(),
            size: file.stat.size,
            modified_at: 0,
            blob: format!("blobs/1-dev/{path}"),
            deleted_at: None,
        };
        manifest.files.insert(path.clone(), entry);
        local.insert(path.clone(), file.stat);
    }
    BaseManifest {
        local,
        ..BaseManifest::new(manifest)
    }
}

#[test]
fn walks_nested_paths_with_slashes_and_skips_exclusions() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "save.srm", b"sram");
    write(root, "PSP/SAVEDATA/ULUS10041/DATA.BIN", b"psp");
    write(root, "User/Cache/Shaders/x.cache", b"cache");
    write(root, "User/GC/MemoryCardA.USA.raw", b"card");
    write(root, "save.srm.tmp", b"t");
    write(root, "slot1.state", b"state");
    let scope = SyncScope {
        sync_states: false,
        ..SyncScope::default()
    };
    let snapshot = snapshot_local_dir(root, None, &scope).unwrap();
    let paths: Vec<&str> = snapshot.files.keys().map(String::as_str).collect();
    assert_eq!(
        paths,
        [
            "PSP/SAVEDATA/ULUS10041/DATA.BIN",
            "User/GC/MemoryCardA.USA.raw",
            "save.srm"
        ]
    );
    assert_eq!(snapshot.hashed, 3);
    let sram = &snapshot.files["save.srm"];
    assert_eq!(sram.sha256.as_deref(), Some(sha256_hex(b"sram").as_str()));
    assert_eq!(sram.stat.size, 4);
}

#[test]
fn hashes_only_files_whose_stat_moved() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, "save.srm", b"one");
    write(root, "rtc.bin", b"clock");
    let scope = SyncScope::default();
    let first = snapshot_local_dir(root, None, &scope).unwrap();
    assert_eq!(first.hashed, 2);
    let base = base_of(&first);

    let again = snapshot_local_dir(root, Some(&base), &scope).unwrap();
    assert_eq!(again.hashed, 0, "stats equal: base shas reused");
    assert_eq!(again.files, first.files);

    write(root, "save.srm", b"longer"); // size moved
    let changed = snapshot_local_dir(root, Some(&base), &scope).unwrap();
    assert_eq!(changed.hashed, 1);
    assert_eq!(
        changed.files["save.srm"].sha256.as_deref(),
        Some(sha256_hex(b"longer").as_str())
    );

    let mut cleared = base.clone();
    cleared.local.clear(); // a cleared stat forces a re-hash
    assert_eq!(
        snapshot_local_dir(root, Some(&cleared), &scope)
            .unwrap()
            .hashed,
        2
    );
}

#[test]
fn large_files_are_listed_but_never_hashed() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "big.state", &[0u8; 64]);
    write(dir.path(), "save.srm", b"x");
    let scope = SyncScope {
        max_file_bytes: 16,
        ..SyncScope::default()
    };
    let snapshot = snapshot_local_dir(dir.path(), None, &scope).unwrap();
    assert_eq!(snapshot.hashed, 1);
    assert_eq!(snapshot.files["big.state"].sha256, None);
    assert_eq!(snapshot.files["big.state"].stat.size, 64);
}

#[test]
fn absent_folder_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let snapshot =
        snapshot_local_dir(&dir.path().join("missing"), None, &SyncScope::default()).unwrap();
    assert_eq!(snapshot, LocalSnapshot::default());
}
