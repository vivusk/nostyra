#![allow(clippy::unwrap_used)] // test code: panics are the failure signal
use super::*;

fn sample() -> Credentials {
    Credentials {
        user: "Tester".into(),
        token: "tok123".into(),
    }
}

#[test]
fn save_load_delete_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(load_credentials(dir.path()).unwrap(), None);
    save_credentials(dir.path(), &sample()).unwrap();
    assert_eq!(load_credentials(dir.path()).unwrap(), Some(sample()));
    let text = std::fs::read_to_string(credentials_path(dir.path())).unwrap();
    assert!(text.contains("\"user\"") && text.contains("\"token\""));
    assert!(!dir
        .path()
        .join("achievements/credentials.json.tmp")
        .exists());
    delete_credentials(dir.path()).unwrap();
    delete_credentials(dir.path()).unwrap();
    assert_eq!(load_credentials(dir.path()).unwrap(), None);
}

#[test]
fn incomplete_file_reads_as_logged_out() {
    let dir = tempfile::tempdir().unwrap();
    let empty_token = Credentials {
        user: "Tester".into(),
        token: " ".into(),
    };
    save_credentials(dir.path(), &empty_token).unwrap();
    assert_eq!(load_credentials(dir.path()).unwrap(), None);
}

#[test]
fn debug_hides_the_token() {
    assert!(!format!("{:?}", sample()).contains("tok123"));
}

#[cfg(unix)]
#[test]
fn file_is_owner_only_on_unix() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    save_credentials(dir.path(), &sample()).unwrap();
    let mode = std::fs::metadata(credentials_path(dir.path()))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

/// `<data>/achievements/offline-cache` with one cached file.
fn seed_offline_cache(data: &Path) -> PathBuf {
    let cache = data.join("achievements").join("offline-cache");
    std::fs::create_dir_all(cache.join("tester")).unwrap();
    std::fs::write(cache.join("tester").join("login2.json"), "{}").unwrap();
    cache
}

#[test]
fn logout_removes_the_offline_cache_even_without_credentials() {
    let dir = tempfile::tempdir().unwrap();
    save_credentials(dir.path(), &sample()).unwrap();
    let cache = seed_offline_cache(dir.path());
    delete_credentials(dir.path()).unwrap();
    assert!(!cache.exists());
    let cache = seed_offline_cache(dir.path());
    delete_credentials(dir.path()).unwrap();
    assert!(!cache.exists(), "no credentials file: cache still removed");
}

#[test]
fn account_switch_clears_the_cache_but_a_relogin_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    save_credentials(dir.path(), &sample()).unwrap();
    let cache = seed_offline_cache(dir.path());
    let same_user = Credentials {
        user: "TESTER".into(),
        token: "new".into(),
    };
    save_credentials(dir.path(), &same_user).unwrap();
    assert!(cache.exists(), "same user, other case: kept");
    let other = Credentials {
        user: "Someone".into(),
        token: "t".into(),
    };
    save_credentials(dir.path(), &other).unwrap();
    assert!(!cache.exists(), "other user: cleared");
    let cache = seed_offline_cache(dir.path());
    std::fs::write(credentials_path(dir.path()), b"not json").unwrap();
    save_credentials(dir.path(), &other).unwrap();
    assert!(!cache.exists(), "unreadable old file: cleared");
}
