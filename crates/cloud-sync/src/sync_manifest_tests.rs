use super::*;

fn entry(sha: &str, blob: &str) -> FileEntry {
    FileEntry {
        sha256: sha.to_string(),
        size: 4,
        modified_at: 10,
        blob: blob.to_string(),
        deleted_at: None,
    }
}

fn sample() -> RemoteManifest {
    let mut manifest = RemoteManifest::empty("0123456789abcdef", "PC-A");
    manifest.generation = 3;
    manifest.updated_at = 1_000;
    manifest
        .files
        .insert("save.srm".into(), entry("aa", "blobs/3-01234567/save.srm"));
    let mut gone = entry("bb", "blobs/1-01234567/slot1.state");
    gone.deleted_at = Some(500);
    manifest.files.insert("slot1.state".into(), gone);
    manifest
}

#[test]
fn json_round_trip() {
    let mut manifest = sample();
    let bytes = manifest.encode(1_000).unwrap();
    assert_eq!(RemoteManifest::parse(&bytes).unwrap(), manifest);
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.contains("\"format\": 1"), "{text}");
    // A live entry carries no `deleted_at` key.
    let live = serde_json::to_string(&manifest.files["save.srm"]).unwrap();
    assert!(!live.contains("deleted_at"), "{live}");
}

#[test]
fn unsupported_or_missing_format_is_refused() {
    let newer = br#"{"format":2,"generation":1,"files":{}}"#;
    assert!(matches!(
        RemoteManifest::parse(newer),
        Err(CloudSyncError::UnsupportedRemoteFormat {
            found: Some(2),
            expected: 1
        })
    ));
    assert!(matches!(
        RemoteManifest::parse(br#"{"generation":1}"#),
        Err(CloudSyncError::UnsupportedRemoteFormat { found: None, .. })
    ));
    assert!(matches!(
        RemoteManifest::parse(b"not json"),
        Err(CloudSyncError::Json { .. })
    ));
}

#[test]
fn tombstones_expire_after_90_days_on_write() {
    let mut manifest = sample();
    manifest.encode(500 + TOMBSTONE_MAX_AGE_SECS).unwrap();
    assert!(
        manifest.files.contains_key("slot1.state"),
        "exactly 90 d: kept"
    );
    let bytes = manifest.encode(501 + TOMBSTONE_MAX_AGE_SECS).unwrap();
    let decoded = RemoteManifest::parse(&bytes).unwrap();
    assert!(!decoded.files.contains_key("slot1.state"));
    assert!(
        decoded.files.contains_key("save.srm"),
        "live files never expire"
    );
}

#[test]
fn same_refs_compares_writer_and_blobs() {
    let base = sample();
    let mut other = base.clone();
    other.generation = 9;
    assert!(base.same_refs(&other), "generation aside");
    other.device_id = "ffffffff00000000".into();
    assert!(!base.same_refs(&other));
    let mut other = base.clone();
    other.files.get_mut("save.srm").unwrap().blob = "blobs/3-ffffffff/save.srm".into();
    assert!(!base.same_refs(&other));
    let mut other = base.clone();
    other.files.get_mut("save.srm").unwrap().deleted_at = Some(1);
    assert!(!base.same_refs(&other));
}

#[test]
fn blob_paths_are_per_generation_and_device() {
    assert_eq!(
        blob_path(4, "0123456789abcdef", "PSP/SAVEDATA/X/DATA.BIN"),
        "blobs/4-01234567/PSP/SAVEDATA/X/DATA.BIN"
    );
}
