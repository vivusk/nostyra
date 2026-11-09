use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::*;
use crate::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};

fn entry(generation: u64) -> IndexEntry {
    IndexEntry {
        generation,
        updated_at: generation,
        device_name: "PC".into(),
        repair: Vec::new(),
    }
}

#[test]
fn absent_index_is_empty_and_other_formats_are_refused() {
    let fake = FakeCloudProvider::new();
    assert_eq!(read_index(&fake).unwrap(), (CloudIndex::default(), None));
    fake.put(INDEX_PATH, br#"{"format":2,"units":{}}"#);
    assert_eq!(read_index(&fake), Err(CloudError::UnsupportedRemoteFormat));
}

#[test]
fn concurrent_writers_merge_per_key() {
    let fake = Arc::new(FakeCloudProvider::new());
    update_index(&*fake, None, |index| {
        index.units.insert("a".into(), entry(1));
    })
    .unwrap();
    // Another PC writes its own key between our read and our write.
    let other = Arc::clone(&fake);
    let fired = Arc::new(AtomicBool::new(false));
    let once = Arc::clone(&fired);
    fake.set_hook(Some(Arc::new(move |op, _| {
        if op == FakeOp::Write && !once.swap(true, Ordering::SeqCst) {
            update_index(&*other, None, |index| {
                index.units.insert("b".into(), entry(7));
            })
            .unwrap();
        }
    })));
    update_index(&*fake, None, |index| {
        index.units.insert("a".into(), entry(2));
    })
    .unwrap();
    fake.set_hook(None);
    let (index, _) = read_index(&*fake).unwrap();
    assert_eq!(index.units["a"].generation, 2);
    assert_eq!(index.units["b"].generation, 7, "merged, not overwritten");
}

#[test]
fn gives_up_after_three_lost_races() {
    let fake = FakeCloudProvider::new();
    fake.fail_next(Some(FakeOp::Write), INDEX_PATH, CloudError::RevMismatch, 3);
    let result = update_index(&fake, None, |index| {
        index.units.insert("a".into(), entry(1));
    });
    assert_eq!(result, Err(CloudError::RevMismatch));
}

#[test]
fn changed_units_follow_generation_and_repair() {
    let mut index = CloudIndex::default();
    index.units.insert("same".into(), entry(3));
    index.units.insert("newer".into(), entry(4));
    index.units.insert("unknown".into(), entry(1));
    let mut flagged = entry(2);
    flagged.repair.push(RepairRef {
        path: "save.srm".into(),
        blob: "blobs/2-aaaaaaaa/save.srm".into(),
    });
    index.units.insert("flagged".into(), flagged);
    let bases = [("same", 3), ("newer", 3), ("flagged", 2)]
        .map(|(key, generation)| (key.to_string(), generation))
        .into();
    assert_eq!(
        changed_units(&index, &bases),
        vec!["flagged", "newer", "unknown"]
    );
}

#[test]
fn mangled_index_resets_but_a_newer_format_is_never_taken_for_garbage() {
    let fake = FakeCloudProvider::new();
    fake.put(INDEX_PATH, b"{ not json");
    assert!(matches!(read_index(&fake), Err(CloudError::Other(_))));
    let (index, rev) = read_index_or_reset(&fake).unwrap();
    assert_eq!(index, CloudIndex::default());
    assert!(
        rev.is_some(),
        "rev kept: the reset is a conditional overwrite"
    );
    fake.put(INDEX_PATH, br#"{"format":2,"units":"a new shape"}"#);
    assert_eq!(
        read_index_or_reset(&fake),
        Err(CloudError::UnsupportedRemoteFormat)
    );
}
