use super::*;
use crate::sync_manifest::FileEntry;

fn unit() -> SyncUnit {
    SyncUnit::game("gba", "ab12cd34ef", PathBuf::from("saves")).unwrap()
}

#[test]
fn missing_base_is_none_and_save_load_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = BaseManifestStore::new(dir.path(), &account_key("local_folder", "x"));
    assert_eq!(store.load(&unit()).unwrap(), None);

    let mut manifest = RemoteManifest::empty("dev", "PC");
    manifest.generation = 2;
    manifest.files.insert(
        "save.srm".into(),
        FileEntry {
            sha256: "aa".into(),
            size: 3,
            modified_at: 1,
            blob: "blobs/2-dev/save.srm".into(),
            deleted_at: None,
        },
    );
    let mut base = BaseManifest::new(manifest);
    base.local.insert(
        "save.srm".into(),
        LocalStat {
            size: 3,
            modified_ms: 7,
        },
    );
    store.save(&unit(), &base).unwrap();
    assert_eq!(store.load(&unit()).unwrap(), Some(base.clone()));
    assert_eq!(base.live_sha("save.srm"), Some("aa"));
    assert_eq!(base.live_sha("other"), None);

    let path = store.path_for(&unit());
    assert!(path.starts_with(dir.path().join("cloud").join("base")));
    assert!(path.ends_with(std::path::Path::new("gba").join("ab12cd34ef.json")));
    assert!(!path.with_extension("json.tmp").exists());

    std::fs::write(&path, b"{broken").unwrap();
    assert_eq!(store.load(&unit()).unwrap(), None, "corrupt base = no base");
}

#[test]
fn shared_unit_path_and_account_keys() {
    let store = BaseManifestStore::new(Path::new("data"), "k");
    let shared = SyncUnit::ps2_memcards(Path::new("data"));
    let path = store.path_for(&shared);
    assert!(path.ends_with(Path::new("k").join("shared").join("ps2-memcards.json")));

    let a = account_key("dropbox", "id-1");
    assert!(a.starts_with("dropbox-") && a.len() == "dropbox-".len() + 16);
    assert_ne!(a, account_key("dropbox", "id-2"));
    assert_ne!(a, account_key("onedrive", "id-1"));
}

#[test]
fn local_folder_account_id_follows_the_folder() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("A");
    let b = dir.path().join("B");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let id = local_folder_account_id(&a);
    assert_eq!(id.len(), 64);
    assert_ne!(id, local_folder_account_id(&b));
    // Same folder through another spelling → same account.
    assert_eq!(
        id,
        local_folder_account_id(&dir.path().join("B").join("..").join("A"))
    );
    if cfg!(windows) {
        let upper = PathBuf::from(a.to_string_lossy().to_uppercase());
        assert_eq!(id, local_folder_account_id(&upper));
    }
}
