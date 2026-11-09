use super::*;

#[test]
fn round_trip_conditional_writes_and_deletes() {
    let dir = tempfile::tempdir().unwrap();
    let provider = LocalFolderProvider::new(dir.path());
    assert_eq!(provider.read("index.json"), Err(CloudError::NotFound));
    assert_eq!(provider.list(""), Err(CloudError::NotFound));

    let rev = provider
        .write("games/x/manifest.json", b"one", None)
        .unwrap();
    assert!(provider.root().join("games/x/manifest.json").is_file());
    let (bytes, read_rev) = provider.read("games/x/manifest.json").unwrap();
    assert_eq!((bytes.as_slice(), &read_rev), (&b"one"[..], &rev));

    let rev2 = provider
        .write("games/x/manifest.json", b"two", Some(&rev))
        .unwrap();
    assert_eq!(
        provider.write("games/x/manifest.json", b"three", Some(&rev)),
        Err(CloudError::RevMismatch),
        "stale rev"
    );
    assert_eq!(
        provider.write("games/x/new.json", b"x", Some(&rev2)),
        Err(CloudError::RevMismatch),
        "expected a file that does not exist"
    );

    let mut reader: &[u8] = b"large blob";
    let blob_rev = provider
        .write_large("games/x/blobs/1-abcdef12/save.srm", &mut reader, 10)
        .unwrap();
    assert_eq!(blob_rev.0, sha256_hex(b"large blob"));
    let names: Vec<_> = provider.list("games/x").unwrap();
    assert_eq!(
        names,
        vec![
            ListEntry {
                name: "blobs".into(),
                is_dir: true
            },
            ListEntry {
                name: "manifest.json".into(),
                is_dir: false
            },
        ]
    );
    provider.delete_dir("games/x/blobs").unwrap();
    provider.delete_dir("games/x/blobs").unwrap();
    provider.delete("games/x/manifest.json").unwrap();
    provider.delete("games/x/manifest.json").unwrap();
    assert_eq!(provider.list("games/x").unwrap(), vec![]);
    assert!(provider.delete_dir("").is_err());
}

#[test]
fn unsafe_paths_and_client_copies() {
    let dir = tempfile::tempdir().unwrap();
    let provider = LocalFolderProvider::new(dir.path());
    for bad in ["../escape", "a/../../b", "/abs", "a\\b"] {
        assert!(matches!(
            provider.write(bad, b"x", None),
            Err(CloudError::Other(_))
        ));
        assert!(matches!(provider.read(bad), Err(CloudError::Other(_))));
    }
    provider.write("d/manifest.json", b"x", None).unwrap();
    for junk in [
        "manifest (conflicted copy 2026-11-09).json",
        "manifest.json.tmp",
    ] {
        std::fs::write(provider.root().join("d").join(junk), b"y").unwrap();
    }
    let pc = current_computer_name();
    std::fs::write(
        provider
            .root()
            .join("d")
            .join(format!("manifest-{pc}.json")),
        b"y",
    )
    .unwrap();
    let names: Vec<String> = provider
        .list("d")
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["manifest.json".to_string()]);
}

#[test]
fn oversized_metadata_is_refused_blobs_still_stream() {
    let dir = tempfile::tempdir().unwrap();
    let provider = LocalFolderProvider::new(dir.path());
    let big = vec![b' '; crate::metadata_read_cap::MAX_METADATA_BYTES as usize + 1];
    provider.write("games/x/manifest.json", &big, None).unwrap();
    let err = provider.read("games/x/manifest.json").unwrap_err();
    assert!(matches!(err, CloudError::Other(msg) if msg.contains("8 MiB")));
    let mut out = Vec::new();
    provider.read_to("games/x/manifest.json", &mut out).unwrap();
    assert_eq!(out.len(), big.len());
}
