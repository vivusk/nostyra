//! Launcher cloud flows over two simulated PCs: pull before play, push after the
//! session, pending changes never uploaded before play (r2 OQ1), conflicts resolved
//! both ways before play (r3 F1) and from the Detail chip.

use std::sync::Arc;

use cloud_sync::providers::fake_cloud_provider::{FakeCloudProvider, FakeOp};
use cloud_sync::{unit_backups_dir, SyncOptions};

use crate::cloud_game_result::SyncGameResult;
use crate::cloud_provider_factory::local_folder;
use crate::cloud_test_harness::{fake_account, generation, TestHost, SHA1_A, SYSTEM};
use crate::commands::cloud_sync_commands::{resolve_options, ConflictChoice};

#[test]
fn pulled_before_play_and_pushed_after_the_session_with_a_local_folder() {
    let folder = tempfile::tempdir().unwrap();
    let account = local_folder(folder.path());
    let (a, b) = (TestHost::new("PC-A"), TestHost::new("PC-B"));
    let (ta, tb) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    assert_ne!(a.save_dir(&ta).file_name(), b.save_dir(&tb).file_name());
    a.write(&ta, "save.srm", b"v1");
    assert_eq!(a.sync_both(&account, &ta), SyncGameResult::Synced);
    assert_eq!(
        b.pre_play(&account, &tb, SyncOptions::pull_only()),
        SyncGameResult::Synced
    );
    assert_eq!(b.read(&tb, "save.srm").unwrap(), b"v1");
    // Session on B writes, then the post-session push; A pulls it before its next play.
    b.start_session(&tb);
    b.write(&tb, "save.srm", b"v2");
    *b.0.session.lock().unwrap() = None;
    assert_eq!(b.sync_both(&account, &tb), SyncGameResult::Synced);
    assert_eq!(
        a.pre_play(&account, &ta, SyncOptions::pull_only()),
        SyncGameResult::Synced
    );
    assert_eq!(a.read(&ta, "save.srm").unwrap(), b"v2");
}

#[test]
fn pending_local_changes_wait_for_the_session_end() {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let (a, b) = (TestHost::new("PC-A"), TestHost::new("PC-B"));
    let (ta, tb) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    let unit = &a.units(&ta)[0];
    a.write(&ta, "save.srm", b"v1");
    a.sync_both(&account, &ta);
    b.pre_play(&account, &tb, SyncOptions::pull_only());
    b.write(&tb, "save.srm", b"v2");
    fake.clear_calls();
    assert_eq!(
        b.pre_play(&account, &tb, SyncOptions::pull_only()),
        SyncGameResult::Synced
    );
    let uploads = fake
        .calls()
        .into_iter()
        .filter(|(op, _)| matches!(op, FakeOp::Write | FakeOp::WriteLarge))
        .count();
    assert_eq!(uploads, 0, "nothing uploaded before play");
    assert_eq!(generation(&fake, unit), 1);
    assert_eq!(b.sync_both(&account, &tb), SyncGameResult::Synced);
    assert_eq!(generation(&fake, unit), 2, "pushed after the session");
}

/// A has v1 in the cloud, B has its own w1 (first sync on B): a conflict.
fn conflicted() -> (Arc<FakeCloudProvider>, TestHost, TestHost) {
    let fake = Arc::new(FakeCloudProvider::new());
    let account = fake_account(&fake);
    let (a, b) = (TestHost::new("PC-A"), TestHost::new("PC-B"));
    let (ta, tb) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    a.write(&ta, "save.srm", b"v1");
    a.sync_both(&account, &ta);
    b.write(&tb, "save.srm", b"w1");
    (fake, a, b)
}

#[test]
fn pre_play_conflict_keep_local_uploads_only_after_the_session() {
    let (fake, a, b) = conflicted();
    let account = fake_account(&fake);
    let (ta, tb) = (a.game(SYSTEM, SHA1_A), b.game(SYSTEM, SHA1_A));
    let unit = b.units(&tb)[0].clone();
    match b.pre_play(&account, &tb, SyncOptions::pull_only()) {
        SyncGameResult::Conflict {
            files,
            local_time,
            remote_time,
            remote_device,
        } => {
            assert_eq!(files, ["save.srm"]);
            assert!(local_time.is_some() && remote_time.is_some());
            assert!(remote_device.is_some());
        }
        other => panic!("{other:?}"),
    }
    let keep = resolve_options(ConflictChoice::KeepLocal, true);
    assert_eq!(b.pre_play(&account, &tb, keep), SyncGameResult::Synced);
    assert_eq!(generation(&fake, &unit), 1, "no upload before play");
    assert_eq!(b.read(&tb, "save.srm").unwrap(), b"w1");
    assert_eq!(
        b.sync_both(&account, &tb),
        SyncGameResult::Synced,
        "no second conflict"
    );
    assert_eq!(generation(&fake, &unit), 2);
    let backups = std::fs::read_dir(unit_backups_dir(b.root(), &unit.key())).unwrap();
    let names: Vec<String> = backups
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        names.iter().any(|name| name.ends_with("-cloud")),
        "{names:?}"
    );
    assert_eq!(
        a.pre_play(&account, &ta, SyncOptions::pull_only()),
        SyncGameResult::Synced
    );
    assert_eq!(a.read(&ta, "save.srm").unwrap(), b"w1");
}

#[test]
fn pre_play_conflict_use_remote_pulls_before_the_runner_starts() {
    let (fake, _a, b) = conflicted();
    let account = fake_account(&fake);
    let tb = b.game(SYSTEM, SHA1_A);
    let unit = b.units(&tb)[0].clone();
    let use_cloud = resolve_options(ConflictChoice::UseRemote, true);
    assert_eq!(b.pre_play(&account, &tb, use_cloud), SyncGameResult::Synced);
    assert_eq!(b.read(&tb, "save.srm").unwrap(), b"v1");
    assert_eq!(generation(&fake, &unit), 1);
    assert!(
        unit_backups_dir(b.root(), &unit.key()).is_dir(),
        "local copy kept"
    );
}

#[test]
fn detail_chip_resolution_is_a_full_sync() {
    let (fake, _a, b) = conflicted();
    let account = fake_account(&fake);
    let tb = b.game(SYSTEM, SHA1_A);
    let unit = b.units(&tb)[0].clone();
    let options = resolve_options(ConflictChoice::KeepLocal, false);
    let request = b.request(&account, &tb, options);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let kind = crate::cloud_sync_state::JobKind::Push;
    let result = crate::cloud_sync_jobs::run_game_job(&b, kind, deadline, stop, &request);
    assert_eq!(result, SyncGameResult::Synced);
    assert_eq!(generation(&fake, &unit), 2, "uploaded right away");
    assert!(b.cloud().status().conflict_games.is_empty());
}
