#![allow(clippy::unwrap_used)] // test code: panics are the failure signal

use super::*;

const HASH: &str = "0123456789ABCDEF0123456789ABCDEF";
const SESSION: &str = r#"{"Success":true,"Unlocks":[],"HardcoreUnlocks":[],"ServerNow":1}"#;

fn cache() -> (tempfile::TempDir, OfflineResponseCache) {
    let data = tempfile::tempdir().unwrap();
    let cache = OfflineResponseCache::new(data.path());
    (data, cache)
}

fn unlock_ids(cache: &OfflineResponseCache, field: &str) -> Vec<u64> {
    let text = cache
        .load(CachedAnswer::Session, "Tester", Some(HASH))
        .unwrap();
    let json: Value = serde_json::from_str(&text).unwrap();
    json[field]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            assert!(entry["When"].is_i64(), "every entry has When: {entry}");
            entry["ID"].as_u64().unwrap()
        })
        .collect()
}

#[test]
fn store_load_and_clear_per_user_and_hash() {
    let (data, cache) = cache();
    cache
        .store(CachedAnswer::Login, "Tester", None, "{\"login\":1}")
        .unwrap();
    cache
        .store(CachedAnswer::GameSets, "Tester", Some(HASH), "{\"set\":1}")
        .unwrap();
    let login = cache.load(CachedAnswer::Login, "Tester", None);
    assert_eq!(login.as_deref(), Some("{\"login\":1}"));
    let lower = HASH.to_ascii_lowercase();
    let set = cache.load(CachedAnswer::GameSets, "tester", Some(&lower));
    assert_eq!(set.as_deref(), Some("{\"set\":1}"), "case of user and hash");
    assert_eq!(
        cache.load(CachedAnswer::Session, "Tester", Some(HASH)),
        None
    );
    assert_eq!(
        cache.load(CachedAnswer::GameSets, "Other", Some(HASH)),
        None
    );
    let root = data.path().join("achievements").join("offline-cache");
    assert!(root
        .join("tester")
        .join("sets")
        .join(format!("{lower}.json"))
        .exists());
    assert!(
        !root.join("tester").join("login2.json.tmp").exists(),
        "atomic write"
    );
    OfflineResponseCache::clear(data.path()).unwrap();
    OfflineResponseCache::clear(data.path()).unwrap();
    assert!(!root.exists());
}

#[test]
fn url_encoded_and_mixed_case_users_share_one_folder() {
    let (_data, cache) = cache();
    cache
        .store(CachedAnswer::Login, "Mixed%2DCase", None, "{}")
        .unwrap();
    assert!(cache
        .load(CachedAnswer::Login, "mixed-case", None)
        .is_some());
    assert!(cache
        .load(CachedAnswer::Login, "MIXED-CASE", None)
        .is_some());
}

#[test]
fn bad_hashes_are_never_cached() {
    let (data, cache) = cache();
    for hash in [None, Some(""), Some("../x"), Some("zz")] {
        cache
            .store(CachedAnswer::GameSets, "Tester", hash, "{}")
            .unwrap();
        assert_eq!(cache.load(CachedAnswer::GameSets, "Tester", hash), None);
    }
    assert!(!data.path().join("achievements").exists());
}

#[test]
fn oversized_files_are_not_served() {
    let (_data, cache) = cache();
    cache
        .store(CachedAnswer::Login, "Tester", None, "{}")
        .unwrap();
    let path = cache.path(CachedAnswer::Login, "Tester", None).unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_len(RESPONSE_LIMIT_BYTES + 1).unwrap();
    assert_eq!(cache.load(CachedAnswer::Login, "Tester", None), None);
}

#[test]
fn record_unlock_adds_each_id_once_per_mode() {
    let (_data, cache) = cache();
    cache.record_unlock("Tester", HASH, 7, false, 100); // no cached session: no-op
    assert_eq!(
        cache.load(CachedAnswer::Session, "Tester", Some(HASH)),
        None
    );
    cache
        .store(CachedAnswer::Session, "Tester", Some(HASH), SESSION)
        .unwrap();
    cache.record_unlock("Tester", HASH, 7, false, 100);
    cache.record_unlock("tester", HASH, 7, false, 200);
    cache.record_unlock("Tester", HASH, 8, true, 300);
    assert_eq!(unlock_ids(&cache, "Unlocks"), [7]);
    assert_eq!(unlock_ids(&cache, "HardcoreUnlocks"), [8]);
}

#[test]
fn add_unlock_creates_a_missing_list() {
    let mut json: Value = serde_json::from_str("{\"Success\":true}").unwrap();
    assert!(add_unlock(&mut json, 5, false, 9));
    assert!(!add_unlock(&mut json, 5, false, 10));
    assert_eq!(json["Unlocks"], json!([{ "ID": 5, "When": 9 }]));
    assert!(!add_unlock(&mut json!([1]), 5, false, 9), "not an object");
}
